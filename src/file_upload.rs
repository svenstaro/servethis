use actix_web::{http::header, HttpRequest, HttpResponse};
use futures::TryStreamExt;
use std::{
    io::Write,
    path::{Component, PathBuf},
};

use crate::errors::ContextualError;
use crate::listing::{self};

/// Create future to save file.
async fn save_file(
    field: actix_multipart::Field,
    file_path: PathBuf,
    overwrite_files: bool,
) -> Result<i64, ContextualError> {
    if !overwrite_files && file_path.exists() {
        return Err(ContextualError::DuplicateFileError);
    }

    let file = std::fs::File::create(&file_path).map_err(|e| {
        ContextualError::IoError(format!("Failed to create {}", file_path.display()), e)
    })?;

    let (_, written_len) = field
        .map_err(ContextualError::MultipartError)
        .try_fold((file, 0i64), |(mut file, written_len), bytes| async move {
            file.write_all(bytes.as_ref())
                .map_err(|e| ContextualError::IoError("Failed to write to file".to_string(), e))?;
            Ok((file, written_len + bytes.len() as i64))
        })
        .await?;

    Ok(written_len)
}

/// Create new future to handle file as multipart data.
async fn handle_multipart(
    field: actix_multipart::Field,
    file_path: PathBuf,
    overwrite_files: bool,
) -> Result<i64, ContextualError> {
    let filename = field
        .content_disposition()
        .and_then(|cd| cd.get_filename().map(String::from))
        .ok_or_else(|| {
            ContextualError::ParseError(
                "HTTP header".to_string(),
                "Failed to retrieve the name of the file to upload".to_string(),
            )
        })?;

    match std::fs::metadata(&file_path) {
        Err(_) => Err(ContextualError::InsufficientPermissionsError(
            file_path.display().to_string(),
        )),
        Ok(metadata) if !metadata.is_dir() => Err(ContextualError::InvalidPathError(format!(
            "cannot upload file to {}, since it's not a directory",
            &file_path.display()
        ))),
        Ok(metadata) if metadata.permissions().readonly() => Err(
            ContextualError::InsufficientPermissionsError(file_path.display().to_string()),
        ),
        Ok(_) => Ok(()),
    }?;

    save_file(field, file_path.join(filename), overwrite_files).await
}

/// Handle incoming request to upload file.
/// Target file path is expected as path parameter in URI and is interpreted as relative from
/// server root directory. Any path which will go outside of this directory is considered
/// invalid.
/// This method returns future.
pub async fn upload_file(
    req: HttpRequest,
    payload: actix_web::web::Payload,
) -> Result<HttpResponse, ContextualError> {
    let conf = req.app_data::<crate::MiniserveConfig>().unwrap();
    let return_path = if let Some(header) = req.headers().get(header::REFERER) {
        header.to_str().unwrap_or("/").to_owned()
    } else {
        "/".to_string()
    };

    let query_params = listing::extract_query_parameters(&req);
    let upload_path = query_params.path.as_ref().ok_or_else(|| {
        ContextualError::InvalidHttpRequestError("Missing query parameter 'path'".to_string())
    })?;
    let upload_path = match upload_path.strip_prefix(Component::RootDir) {
        Ok(stripped_path) => stripped_path,
        Err(_) => upload_path,
    };

    let app_root_dir = conf.path.canonicalize().map_err(|e| {
        ContextualError::IoError("Failed to resolve path served by miniserve".to_string(), e)
    })?;

    // If the target path is under the app root directory, save the file.
    let target_dir = match app_root_dir.join(upload_path).canonicalize() {
        Ok(path) if path.starts_with(&app_root_dir) => Ok(path),
        _ => Err(ContextualError::InvalidHttpRequestError(
            "Invalid value for 'path' parameter".to_string(),
        )),
    }?;

    actix_multipart::Multipart::new(req.headers(), payload)
        .map_err(ContextualError::MultipartError)
        .and_then(|item| handle_multipart(item, target_dir.clone(), conf.overwrite_files))
        .try_collect::<Vec<i64>>()
        .await?;

    Ok(HttpResponse::SeeOther()
        .header(header::LOCATION, return_path)
        .finish())
}
