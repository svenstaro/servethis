use actix_web::{
    http::{header, StatusCode},
    HttpRequest, HttpResponse,
};
use futures::{future, Future, FutureExt, Stream, TryStreamExt};
use std::{
    io::Write,
    path::{Component, PathBuf},
    pin::Pin,
};

use crate::errors::{self, ContextualError};
use crate::listing::{self, SortingMethod, SortingOrder};
use crate::renderer;

/// Create future to save file.
fn save_file(
    field: actix_multipart::Field,
    file_path: PathBuf,
    overwrite_files: bool,
) -> Pin<Box<dyn Future<Output = Result<i64, ContextualError>>>> {
    if !overwrite_files && file_path.exists() {
        return Box::pin(future::err(ContextualError::DuplicateFileError));
    }

    let mut file = match std::fs::File::create(&file_path) {
        Ok(file) => file,
        Err(e) => {
            return Box::pin(future::err(ContextualError::IoError(
                format!("Failed to create {}", file_path.display()),
                e,
            )));
        }
    };
    Box::pin(
        field
            .map_err(ContextualError::MultipartError)
            .try_fold(0i64, move |acc, bytes| {
                let rt = file
                    .write_all(bytes.as_ref())
                    .map(|_| acc + bytes.len() as i64)
                    .map_err(|e| {
                        ContextualError::IoError("Failed to write to file".to_string(), e)
                    });
                future::ready(rt)
            }),
    )
}

/// Create new future to handle file as multipart data.
fn handle_multipart(
    field: actix_multipart::Field,
    mut file_path: PathBuf,
    overwrite_files: bool,
) -> Pin<Box<dyn Stream<Item = Result<i64, ContextualError>>>> {
    let filename = field
        .headers()
        .get(header::CONTENT_DISPOSITION)
        .ok_or(ContextualError::ParseError)
        .and_then(|cd| {
            header::ContentDisposition::from_raw(cd).map_err(|_| ContextualError::ParseError)
        })
        .and_then(|content_disposition| {
            content_disposition
                .get_filename()
                .ok_or(ContextualError::ParseError)
                .map(String::from)
        });
    let err = |e: ContextualError| Box::pin(future::err(e).into_stream());
    match filename {
        Ok(f) => {
            match std::fs::metadata(&file_path) {
                Ok(metadata) => {
                    if !metadata.is_dir() {
                        return err(ContextualError::InvalidPathError(format!(
                            "cannot upload file to {}, since it's not a directory",
                            &file_path.display()
                        )));
                    } else if metadata.permissions().readonly() {
                        return err(ContextualError::InsufficientPermissionsError(
                            file_path.display().to_string(),
                        ));
                    }
                }
                Err(_) => {
                    return err(ContextualError::InsufficientPermissionsError(
                        file_path.display().to_string(),
                    ));
                }
            }
            file_path = file_path.join(f);
            Box::pin(save_file(field, file_path, overwrite_files).into_stream())
        }
        Err(e) => err(e(
            "HTTP header".to_string(),
            "Failed to retrieve the name of the file to upload".to_string(),
        )),
    }
}

/// Guarantee that the path is relative and cannot traverse back to parent directories
/// and optionally prevent traversing hidden directories.
fn sanitize_path(path: PathBuf, traverse_hidden: bool) -> Option<PathBuf> {
    let mut buf = PathBuf::new();

    for comp in path.components() {
        match comp {
            Component::Normal(name) => buf.push(name),
            Component::ParentDir => {
                buf.pop();
            }
            _ => (),
        }
    }

    // Double-check that all components are Normal and check for hidden dirs
    for comp in buf.components() {
        match comp {
            Component::Normal(_) if traverse_hidden => (),
            Component::Normal(name) if !name.to_str()?.starts_with('.') => (),
            _ => return None,
        }
    }

    Some(buf)
}

/// Handle incoming request to upload file.
/// Target file path is expected as path parameter in URI and is interpreted as relative from
/// server root directory. Any path which will go outside of this directory is considered
/// invalid.
/// This method returns future.
#[allow(clippy::too_many_arguments)]
pub fn upload_file(
    req: HttpRequest,
    payload: actix_web::web::Payload,
    uses_random_route: bool,
    favicon_route: String,
    css_route: String,
    default_color_scheme: &str,
    default_color_scheme_dark: &str,
    hide_version_footer: bool,
) -> Pin<Box<dyn Future<Output = Result<HttpResponse, actix_web::Error>>>> {
    let conf = req.app_data::<crate::MiniserveConfig>().unwrap();
    let return_path = if let Some(header) = req.headers().get(header::REFERER) {
        header.to_str().unwrap_or("/").to_owned()
    } else {
        "/".to_string()
    };

    let query_params = listing::extract_query_parameters(&req);
    let upload_path = match query_params.path.clone() {
        Some(path) => path,
        None => {
            let err = ContextualError::InvalidHttpRequestError(
                "Missing query parameter 'path'".to_string(),
            );
            return Box::pin(create_error_response(
                &err.to_string(),
                StatusCode::BAD_REQUEST,
                &return_path,
                query_params.sort,
                query_params.order,
                uses_random_route,
                &favicon_route,
                &css_route,
                default_color_scheme,
                default_color_scheme_dark,
                hide_version_footer,
            ));
        }
    };

    let upload_path = match sanitize_path(upload_path, conf.show_hidden) {
        Some(path) => path,
        None => {
            let err = ContextualError::InvalidHttpRequestError(
                "Invalid value for 'path' parameter".to_string(),
            );
            return Box::pin(create_error_response(
                &err.to_string(),
                StatusCode::BAD_REQUEST,
                &return_path,
                query_params.sort,
                query_params.order,
                uses_random_route,
                &favicon_route,
                &css_route,
                default_color_scheme,
                default_color_scheme_dark,
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
            return Box::pin(create_error_response(
                &err.to_string(),
                StatusCode::INTERNAL_SERVER_ERROR,
                &return_path,
                query_params.sort,
                query_params.order,
                uses_random_route,
                &favicon_route,
                &css_route,
                default_color_scheme,
                default_color_scheme_dark,
                hide_version_footer,
            ));
        }
    };

    // if no_symlinks, make sure the target dir is under the app root directory
    let target_dir = match &app_root_dir.join(upload_path).canonicalize() {
        Ok(path) if !conf.no_symlinks => path.clone(),
        Ok(path) if path.starts_with(&app_root_dir) => path.clone(),
        _ => {
            let err = ContextualError::InvalidHttpRequestError(
                "Invalid value for 'path' parameter".to_string(),
            );
            return Box::pin(create_error_response(
                &err.to_string(),
                StatusCode::BAD_REQUEST,
                &return_path,
                query_params.sort,
                query_params.order,
                uses_random_route,
                &favicon_route,
                &css_route,
                default_color_scheme,
                default_color_scheme_dark,
                hide_version_footer,
            ));
        }
    };
    let overwrite_files = conf.overwrite_files;
    let default_color_scheme = conf.default_color_scheme.clone();
    let default_color_scheme_dark = conf.default_color_scheme_dark.clone();

    Box::pin(
        actix_multipart::Multipart::new(req.headers(), payload)
            .map_err(ContextualError::MultipartError)
            .map_ok(move |item| handle_multipart(item, target_dir.clone(), overwrite_files))
            .try_flatten()
            .try_collect::<Vec<_>>()
            .then(move |e| match e {
                Ok(_) => future::ok(
                    HttpResponse::SeeOther()
                        .header(header::LOCATION, return_path)
                        .finish(),
                ),
                Err(e) => create_error_response(
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
                ),
            }),
    )
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
) -> future::Ready<Result<HttpResponse, actix_web::Error>> {
    errors::log_error_chain(description.to_string());
    future::ok(
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
                    favicon_route,
                    css_route,
                    default_color_scheme,
                    default_color_scheme_dark,
                    hide_version_footer,
                )
                .into_string(),
            ),
    )
}
