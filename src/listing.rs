use actix_web::body::Body;
use actix_web::dev::ServiceResponse;
use actix_web::http::StatusCode;
use actix_web::web::Query;
use actix_web::{HttpRequest, HttpResponse, Result};
use bytesize::ByteSize;
use percent_encoding::{percent_decode_str, utf8_percent_encode};
use qrcodegen::{QrCode, QrCodeEcc};
use serde::Deserialize;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;
use strum_macros::{Display, EnumString};

use crate::archive::ArchiveMethod;
use crate::errors::{self, ContextualError};
use crate::renderer;
use percent_encode_sets::PATH_SEGMENT;

/// "percent-encode sets" as defined by WHATWG specs:
/// https://url.spec.whatwg.org/#percent-encoded-bytes
mod percent_encode_sets {
    use percent_encoding::{AsciiSet, CONTROLS};
    const BASE: &AsciiSet = &CONTROLS.add(b'%');
    pub const QUERY: &AsciiSet = &BASE.add(b' ').add(b'"').add(b'#').add(b'<').add(b'>');
    pub const PATH: &AsciiSet = &QUERY.add(b'?').add(b'`').add(b'{').add(b'}');
    pub const PATH_SEGMENT: &AsciiSet = &PATH.add(b'/');
}

/// Query parameters
#[derive(Deserialize)]
pub struct QueryParameters {
    pub path: Option<PathBuf>,
    pub sort: Option<SortingMethod>,
    pub order: Option<SortingOrder>,
    qrcode: Option<String>,
    download: Option<ArchiveMethod>,
}

/// Form parameters
#[derive(Deserialize)]
pub struct FormParameters {
    pub mkdir_name: Option<PathBuf>,
}

/// Available sorting methods
#[derive(Deserialize, Clone, EnumString, Display, Copy)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum SortingMethod {
    /// Sort by name
    Name,

    /// Sort by size
    Size,

    /// Sort by last modification date (natural sort: follows alphanumerical order)
    Date,
}

/// Available sorting orders
#[derive(Deserialize, Clone, EnumString, Display, Copy)]
pub enum SortingOrder {
    /// Ascending order
    #[serde(alias = "asc")]
    #[strum(serialize = "asc")]
    Ascending,

    /// Descending order
    #[serde(alias = "desc")]
    #[strum(serialize = "desc")]
    Descending,
}

#[derive(PartialEq)]
/// Possible entry types
pub enum EntryType {
    /// Entry is a directory
    Directory,

    /// Entry is a file
    File,
}

/// Entry
pub struct Entry {
    /// Name of the entry
    pub name: String,

    /// Type of the entry
    pub entry_type: EntryType,

    /// Entry is symlink. Not mutually exclusive with entry_type
    pub is_symlink: bool,

    /// URL of the entry
    pub link: String,

    /// Size in byte of the entry. Only available for EntryType::File
    pub size: Option<bytesize::ByteSize>,

    /// Last modification date
    pub last_modification_date: Option<SystemTime>,
}

impl Entry {
    fn new(
        name: String,
        entry_type: EntryType,
        is_symlink: bool,
        link: String,
        size: Option<bytesize::ByteSize>,
        last_modification_date: Option<SystemTime>,
    ) -> Self {
        Entry {
            name,
            entry_type,
            is_symlink,
            link,
            size,
            last_modification_date,
        }
    }

    /// Returns wether the entry is a directory
    pub fn is_dir(&self) -> bool {
        self.entry_type == EntryType::Directory
    }

    /// Returns wether the entry is a file
    pub fn is_file(&self) -> bool {
        self.entry_type == EntryType::File
    }
}

/// One entry in the path to the listed directory
pub struct Breadcrumb {
    /// Name of directory
    pub name: String,

    /// Link to get to directory, relative to listed directory
    pub link: String,
}

impl Breadcrumb {
    fn new(name: String, link: String) -> Self {
        Breadcrumb { name, link }
    }
}

pub async fn file_handler(req: HttpRequest) -> Result<actix_files::NamedFile> {
    let path = &req.app_data::<crate::MiniserveConfig>().unwrap().path;
    actix_files::NamedFile::open(path).map_err(Into::into)
}

/// List a directory and renders a HTML file accordingly
/// Adapted from https://docs.rs/actix-web/0.7.13/src/actix_web/fs.rs.html#564
#[allow(clippy::too_many_arguments)]
pub fn directory_listing(
    dir: &actix_files::Directory,
    req: &HttpRequest,
    skip_symlinks: bool,
    show_hidden: bool,
    file_upload: bool,
    mkdir: bool,
    random_route: Option<String>,
    favicon_route: String,
    css_route: String,
    default_color_scheme: &str,
    default_color_scheme_dark: &str,
    show_qrcode: bool,
    upload_route: String,
    mkdir_route: String,
    tar_enabled: bool,
    tar_gz_enabled: bool,
    zip_enabled: bool,
    dirs_first: bool,
    hide_version_footer: bool,
    title: Option<String>,
) -> Result<ServiceResponse, io::Error> {
    use actix_web::dev::BodyEncoding;
    let serve_path = req.path();

    // In case the current path is a directory, we want to make sure that the current URL ends
    // on a slash ("/").
    if !serve_path.ends_with('/') {
        let query = match req.query_string() {
            "" => String::new(),
            _ => format!("?{}", req.query_string()),
        };
        return Ok(ServiceResponse::new(
            req.clone(),
            HttpResponse::MovedPermanently()
                .header("Location", format!("{}/{}", serve_path, query))
                .body("301"),
        ));
    }

    let base = Path::new(serve_path);
    let random_route_abs = format!("/{}", random_route.clone().unwrap_or_default());
    let is_root = base.parent().is_none() || Path::new(&req.path()) == Path::new(&random_route_abs);

    let encoded_dir = match base.strip_prefix(random_route_abs) {
        Ok(c_d) => Path::new("/").join(c_d),
        Err(_) => base.to_path_buf(),
    }
    .display()
    .to_string();

    let breadcrumbs = {
        let title = title.unwrap_or_else(|| req.connection_info().host().into());

        let decoded = percent_decode_str(&encoded_dir).decode_utf8_lossy();

        let mut res: Vec<Breadcrumb> = Vec::new();
        let mut link_accumulator =
            format!("/{}", random_route.map(|r| r + "/").unwrap_or_default());

        let mut components = Path::new(&*decoded).components().peekable();

        while let Some(c) = components.next() {
            let name;

            match c {
                Component::RootDir => {
                    name = title.clone();
                }
                Component::Normal(s) => {
                    name = s.to_string_lossy().to_string();
                    link_accumulator
                        .push_str(&(utf8_percent_encode(&name, PATH_SEGMENT).to_string() + "/"));
                }
                _ => name = "".to_string(),
            };

            res.push(Breadcrumb::new(
                name,
                if components.peek().is_some() {
                    link_accumulator.clone()
                } else {
                    ".".to_string()
                },
            ));
        }
        res
    };

    let query_params = extract_query_parameters(req);

    // If the `qrcode` parameter is included in the url, then should respond to the QR code
    if let Some(url) = query_params.qrcode {
        let res = match QrCode::encode_text(&url, QrCodeEcc::Medium) {
            Ok(qr) => HttpResponse::Ok()
                .header("Content-Type", "image/svg+xml")
                .body(qr.to_svg_string(2)),
            Err(err) => {
                log::error!("URL is too long: {:?}", err);
                HttpResponse::UriTooLong().body(Body::Empty)
            }
        };
        return Ok(ServiceResponse::new(req.clone(), res));
    }

    let mut entries: Vec<Entry> = Vec::new();

    for entry in dir.path.read_dir()? {
        if dir.is_visible(&entry) || show_hidden {
            let entry = entry?;
            // show file url as relative to static path
            let file_name = entry.file_name().to_string_lossy().to_string();
            let (is_symlink, metadata) = match entry.metadata() {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    // for symlinks, get the metadata of the original file
                    (true, std::fs::metadata(entry.path()))
                }
                res => (false, res),
            };
            let file_url = base
                .join(&utf8_percent_encode(&file_name, PATH_SEGMENT).to_string())
                .to_string_lossy()
                .to_string();

            // if file is a directory, add '/' to the end of the name
            if let Ok(metadata) = metadata {
                if skip_symlinks && is_symlink {
                    continue;
                }
                let last_modification_date = match metadata.modified() {
                    Ok(date) => Some(date),
                    Err(_) => None,
                };

                if metadata.is_dir() {
                    entries.push(Entry::new(
                        file_name,
                        EntryType::Directory,
                        is_symlink,
                        file_url,
                        None,
                        last_modification_date,
                    ));
                } else if metadata.is_file() {
                    entries.push(Entry::new(
                        file_name,
                        EntryType::File,
                        is_symlink,
                        file_url,
                        Some(ByteSize::b(metadata.len())),
                        last_modification_date,
                    ));
                }
            } else {
                continue;
            }
        }
    }

    match query_params.sort.unwrap_or(SortingMethod::Name) {
        SortingMethod::Name => entries.sort_by(|e1, e2| {
            alphanumeric_sort::compare_str(e1.name.to_lowercase(), e2.name.to_lowercase())
        }),
        SortingMethod::Size => entries.sort_by(|e1, e2| {
            // If we can't get the size of the entry (directory for instance)
            // let's consider it's 0b
            e2.size
                .unwrap_or_else(|| ByteSize::b(0))
                .cmp(&e1.size.unwrap_or_else(|| ByteSize::b(0)))
        }),
        SortingMethod::Date => entries.sort_by(|e1, e2| {
            // If, for some reason, we can't get the last modification date of an entry
            // let's consider it was modified on UNIX_EPOCH (01/01/19270 00:00:00)
            e2.last_modification_date
                .unwrap_or(SystemTime::UNIX_EPOCH)
                .cmp(&e1.last_modification_date.unwrap_or(SystemTime::UNIX_EPOCH))
        }),
    };

    if let Some(SortingOrder::Descending) = query_params.order {
        entries.reverse()
    }

    // List directories first
    if dirs_first {
        entries.sort_by_key(|e| !e.is_dir());
    }

    if let Some(archive_method) = query_params.download {
        if !archive_method.is_enabled(tar_enabled, tar_gz_enabled, zip_enabled) {
            return Ok(ServiceResponse::new(
                req.clone(),
                HttpResponse::Forbidden()
                    .content_type("text/html; charset=utf-8")
                    .body(
                        renderer::render_error(
                            "Archive creation is disabled.",
                            StatusCode::FORBIDDEN,
                            "/",
                            None,
                            None,
                            false,
                            false,
                            &favicon_route,
                            &css_route,
                            default_color_scheme,
                            default_color_scheme_dark,
                            hide_version_footer,
                        )
                        .into_string(),
                    ),
            ));
        }
        log::info!(
            "Creating an archive ({extension}) of {path}...",
            extension = archive_method.extension(),
            path = &dir.path.display().to_string()
        );

        let file_name = format!(
            "{}.{}",
            dir.path.file_name().unwrap().to_str().unwrap(),
            archive_method.extension()
        );

        // We will create the archive in a separate thread, and stream the content using a pipe.
        // The pipe is made of a futures channel, and an adapter to implement the `Write` trait.
        // Include 10 messages of buffer for erratic connection speeds.
        let (tx, rx) = futures::channel::mpsc::channel::<Result<actix_web::web::Bytes, ()>>(10);
        let pipe = crate::pipe::Pipe::new(tx);

        // Start the actual archive creation in a separate thread.
        let dir = dir.path.to_path_buf();
        std::thread::spawn(move || {
            if let Err(err) = archive_method.create_archive(dir, skip_symlinks, pipe) {
                log::error!("Error during archive creation: {:?}", err);
            }
        });

        Ok(ServiceResponse::new(
            req.clone(),
            HttpResponse::Ok()
                .content_type(archive_method.content_type())
                .encoding(archive_method.content_encoding())
                .header("Content-Transfer-Encoding", "binary")
                .header(
                    "Content-Disposition",
                    format!("attachment; filename={:?}", file_name),
                )
                .body(actix_web::body::BodyStream::new(rx)),
        ))
    } else {
        Ok(ServiceResponse::new(
            req.clone(),
            HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(
                    renderer::page(
                        entries,
                        is_root,
                        query_params.sort,
                        query_params.order,
                        show_qrcode,
                        file_upload,
                        mkdir,
                        &upload_route,
                        &mkdir_route,
                        &favicon_route,
                        &css_route,
                        default_color_scheme,
                        default_color_scheme_dark,
                        &encoded_dir,
                        breadcrumbs,
                        tar_enabled,
                        tar_gz_enabled,
                        zip_enabled,
                        hide_version_footer,
                    )
                    .into_string(),
                ),
        ))
    }
}

pub fn extract_query_parameters(req: &HttpRequest) -> QueryParameters {
    match Query::<QueryParameters>::from_query(req.query_string()) {
        Ok(query) => QueryParameters {
            sort: query.sort,
            order: query.order,
            download: query.download,
            qrcode: query.qrcode.to_owned(),
            path: query.path.clone(),
        },
        Err(e) => {
            let err = ContextualError::ParseError("query parameters".to_string(), e.to_string());
            errors::log_error_chain(err.to_string());
            QueryParameters {
                sort: None,
                order: None,
                download: None,
                qrcode: None,
                path: None,
            }
        }
    }
}
