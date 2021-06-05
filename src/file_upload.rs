use actix_web::{
    http::{header, StatusCode},
    HttpRequest, HttpResponse,
};
use futures::TryStreamExt;
use std::{
    io::Write,
    path::{Component, PathBuf},
};

use crate::errors::{self, ContextualError};
use crate::listing::{self, SortingMethod, SortingOrder};
use crate::renderer;

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
#[allow(clippy::too_many_arguments)]
pub async fn upload_file(
    req: HttpRequest,
    payload: actix_web::web::Payload,
    uses_random_route: bool,
    favicon_route: String,
    css_route: String,
    default_color_scheme: String,
    default_color_scheme_dark: String,
    hide_version_footer: bool,
) -> Result<HttpResponse, actix_web::Error> {
    let conf = req.app_data::<crate::MiniserveConfig>().unwrap();
    let return_path = if let Some(header) = req.headers().get(header::REFERER) {
        header.to_str().unwrap_or("/").to_owned()
    } else {
        "/".to_string()
    };

    let query_params = listing::extract_query_parameters(&req);
    let upload_path = match query_params.path.clone() {
        Some(path) => match path.strip_prefix(Component::RootDir) {
            Ok(stripped_path) => stripped_path.to_owned(),
            Err(_) => path.clone(),
        },
        None => {
            let err = ContextualError::InvalidHttpRequestError(
                "Missing query parameter 'path'".to_string(),
            );
            return Ok(create_error_response(
                &err.to_string(),
                StatusCode::BAD_REQUEST,
                &return_path,
                query_params.sort,
                query_params.order,
                uses_random_route,
                &favicon_route,
                &css_route,
                &default_color_scheme,
                &default_color_scheme_dark,
                hide_version_footer,
            ));
        }
    };

    let app_root_dir = match conf.path.canonicalize() {
        Ok(dir) => dir,
        Err(e) => {
            let err = ContextualError::IoError(
                "Failed to resolve path served by miniserve".to_string(),
                e,
            );
            return Ok(create_error_response(
                &err.to_string(),
                StatusCode::INTERNAL_SERVER_ERROR,
                &return_path,
                query_params.sort,
                query_params.order,
                uses_random_route,
                &favicon_route,
                &css_route,
                &default_color_scheme,
                &default_color_scheme_dark,
                hide_version_footer,
            ));
        }
    };

    // If the target path is under the app root directory, save the file.
    let target_dir = match &app_root_dir.join(upload_path).canonicalize() {
        Ok(path) if path.starts_with(&app_root_dir) => path.clone(),
        _ => {
            let err = ContextualError::InvalidHttpRequestError(
                "Invalid value for 'path' parameter".to_string(),
            );
            return Ok(create_error_response(
                &err.to_string(),
                StatusCode::BAD_REQUEST,
                &return_path,
                query_params.sort,
                query_params.order,
                uses_random_route,
                &favicon_route,
                &css_route,
                &default_color_scheme,
                &default_color_scheme_dark,
                hide_version_footer,
            ));
        }
    };
    let overwrite_files = conf.overwrite_files;
    let default_color_scheme = conf.default_color_scheme.clone();
    let default_color_scheme_dark = conf.default_color_scheme_dark.clone();

    match actix_multipart::Multipart::new(req.headers(), payload)
        .map_err(ContextualError::MultipartError)
        .and_then(move |item| handle_multipart(item, target_dir.clone(), overwrite_files))
        .try_collect::<Vec<i64>>()
        .await
    {
        Ok(_) => Ok(HttpResponse::SeeOther()
            .header(header::LOCATION, return_path)
            .finish()),
        Err(e) => Ok(create_error_response(
            &e.to_string(),
            StatusCode::INTERNAL_SERVER_ERROR,
            &return_path,
            query_params.sort,
            query_params.order,
            uses_random_route,
            &favicon_route,
            &css_route,
            &default_color_scheme,
            &default_color_scheme_dark,
            hide_version_footer,
        )),
    }
}

/// Convenience method for creating response errors, if file upload fails.
#[allow(clippy::too_many_arguments)]
fn create_error_response(
    description: &str,
    error_code: StatusCode,
    return_path: &str,
    sorting_method: Option<SortingMethod>,
    sorting_order: Option<SortingOrder>,
    uses_random_route: bool,
    favicon_route: &str,
    css_route: &str,
    default_color_scheme: &str,
    default_color_scheme_dark: &str,
    hide_version_footer: bool,
) -> HttpResponse {
    errors::log_error_chain(description.to_string());
    HttpResponse::BadRequest()
        .content_type("text/html; charset=utf-8")
        .body(
            renderer::render_error(
                description,
                error_code,
                return_path,
                sorting_method,
                sorting_order,
                true,
                !uses_random_route,
                &favicon_route,
                &css_route,
                default_color_scheme,
                default_color_scheme_dark,
                hide_version_footer,
            )
            .into_string(),
        )
}
