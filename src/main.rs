use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::thread;
use std::time::Duration;
use std::{io::Write, path::PathBuf};

use actix_web::web;
use actix_web::{http::header::ContentType, Responder};
use actix_web::{middleware, App, HttpRequest, HttpResponse};
use actix_web_httpauth::middleware::HttpAuthentication;
use http::header::HeaderMap;
use log::{error, warn};
use structopt::clap::crate_version;
use structopt::StructOpt;
use yansi::{Color, Paint};

mod archive;
mod args;
mod auth;
mod errors;
mod file_upload;
mod listing;
mod pipe;
mod renderer;

use crate::errors::ContextualError;

/// Possible characters for random routes
const ROUTE_ALPHABET: [char; 16] = [
    '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', 'a', 'b', 'c', 'd', 'e', 'f',
];

#[derive(Clone)]
/// Configuration of the Miniserve application
pub struct MiniserveConfig {
    /// Enable verbose mode
    pub verbose: bool,

    /// Path to be served by miniserve
    pub path: std::path::PathBuf,

    /// Port on which miniserve will be listening
    pub port: u16,

    /// IP address(es) on which miniserve will be available
    pub interfaces: Vec<IpAddr>,

    /// Enable HTTP basic authentication
    pub auth: Vec<auth::RequiredAuth>,

    /// If false, miniserve will serve the current working directory
    pub path_explicitly_chosen: bool,

    /// Enable symlink resolution
    pub no_symlinks: bool,

    /// Show hidden files
    pub show_hidden: bool,

    /// Enable random route generation
    pub random_route: Option<String>,

    /// Randomly generated favicon route
    pub favicon_route: String,

    /// Randomly generated css route
    pub css_route: String,

    /// Default color scheme
    pub default_color_scheme: String,

    /// Default dark mode color scheme
    pub default_color_scheme_dark: String,

    /// The name of a directory index file to serve, like "index.html"
    ///
    /// Normally, when miniserve serves a directory, it creates a listing for that directory.
    /// However, if a directory contains this file, miniserve will serve that file instead.
    pub index: Option<std::path::PathBuf>,

    /// Enable QR code display
    pub show_qrcode: bool,

    /// Enable file upload
    pub file_upload: bool,

    /// Enable upload to override existing files
    pub overwrite_files: bool,

    /// If false, creation of uncompressed tar archives is disabled
    pub tar_enabled: bool,

    /// If false, creation of gz-compressed tar archives is disabled
    pub tar_gz_enabled: bool,

    /// If false, creation of zip archives is disabled
    pub zip_enabled: bool,

    /// If enabled, directories are listed first
    pub dirs_first: bool,

    /// Shown instead of host in page title and heading
    pub title: Option<String>,

    /// If specified, header will be added
    pub header: Vec<HeaderMap>,

    /// If enabled, version footer is hidden
    pub hide_version_footer: bool,
}

impl MiniserveConfig {
    /// Parses the command line arguments
    fn from_args(args: args::CliArgs) -> Self {
        let interfaces = if !args.interfaces.is_empty() {
            args.interfaces
        } else {
            vec![
                IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)),
                IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            ]
        };

        let random_route = if args.random_route {
            Some(nanoid::nanoid!(6, &ROUTE_ALPHABET))
        } else {
            None
        };

        // Generate some random routes for the favicon and css so that they are very unlikely to conflict with
        // real files.
        let favicon_route = nanoid::nanoid!(10, &ROUTE_ALPHABET);
        let css_route = nanoid::nanoid!(10, &ROUTE_ALPHABET);

        let default_color_scheme = args.color_scheme;
        let default_color_scheme_dark = args.color_scheme_dark;

        let path_explicitly_chosen = args.path.is_some() || args.index.is_some();

        let port = match args.port {
            0 => port_check::free_local_port().expect("no free ports available"),
            _ => args.port,
        };

        crate::MiniserveConfig {
            verbose: args.verbose,
            path: args.path.unwrap_or_else(|| PathBuf::from(".")),
            port,
            interfaces,
            auth: args.auth,
            path_explicitly_chosen,
            no_symlinks: args.no_symlinks,
            show_hidden: args.hidden,
            random_route,
            favicon_route,
            css_route,
            default_color_scheme,
            default_color_scheme_dark,
            index: args.index,
            overwrite_files: args.overwrite_files,
            show_qrcode: args.qrcode,
            file_upload: args.file_upload,
            tar_enabled: args.enable_tar,
            tar_gz_enabled: args.enable_tar_gz,
            zip_enabled: args.enable_zip,
            dirs_first: args.dirs_first,
            title: args.title,
            header: args.header,
            hide_version_footer: args.hide_version_footer,
        }
    }
}

fn main() {
    let args = args::CliArgs::from_args();

    if let Some(shell) = args.print_completions {
        args::CliArgs::clap().gen_completions_to("miniserve", shell, &mut std::io::stdout());
        return;
    }

    let miniserve_config = MiniserveConfig::from_args(args);

    match run(miniserve_config) {
        Ok(()) => (),
        Err(e) => errors::log_error_chain(e.to_string()),
    }
}

#[actix_web::main(miniserve)]
async fn run(miniserve_config: MiniserveConfig) -> Result<(), ContextualError> {
    if cfg!(windows) && !Paint::enable_windows_ascii() {
        Paint::disable();
    }

    let log_level = if miniserve_config.verbose {
        simplelog::LevelFilter::Info
    } else {
        simplelog::LevelFilter::Warn
    };

    if simplelog::TermLogger::init(
        log_level,
        simplelog::Config::default(),
        simplelog::TerminalMode::Mixed,
        simplelog::ColorChoice::Auto,
    )
    .is_err()
    {
        simplelog::SimpleLogger::init(log_level, simplelog::Config::default())
            .expect("Couldn't initialize logger")
    }

    if miniserve_config.no_symlinks {
        let is_symlink = miniserve_config
            .path
            .symlink_metadata()
            .map_err(|e| {
                ContextualError::IoError("Failed to retrieve symlink's metadata".to_string(), e)
            })?
            .file_type()
            .is_symlink();

        if is_symlink {
            return Err(ContextualError::NoSymlinksOptionWithSymlinkServePath(
                miniserve_config.path.to_string_lossy().to_string(),
            ));
        }
    }

    let inside_config = miniserve_config.clone();

    let interfaces = miniserve_config
        .interfaces
        .iter()
        .map(|&interface| {
            if interface == IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)) {
                // If the interface is 0.0.0.0, we'll change it to 127.0.0.1 so that clicking the link will
                // also work on Windows. Why can't Windows interpret 0.0.0.0?
                "127.0.0.1".to_string()
            } else if interface.is_ipv6() {
                // If the interface is IPv6 then we'll print it with brackets so that it is clickable.
                format!("[{}]", interface)
            } else {
                format!("{}", interface)
            }
        })
        .collect::<Vec<String>>();

    let canon_path = miniserve_config.path.canonicalize().map_err(|e| {
        ContextualError::IoError("Failed to resolve path to be served".to_string(), e)
    })?;

    if let Some(index_path) = &miniserve_config.index {
        let has_index: std::path::PathBuf = [&canon_path, index_path].iter().collect();
        if !has_index.exists() {
            error!(
                "The file '{}' provided for option --index could not be found.",
                index_path.to_string_lossy()
            );
        }
    }
    let path_string = canon_path.to_string_lossy();

    println!(
        "{name} v{version}",
        name = Paint::new("miniserve").bold(),
        version = crate_version!()
    );
    if !miniserve_config.path_explicitly_chosen {
        // If the path to serve has NOT been explicitly chosen and if this is NOT an interactive
        // terminal, we should refuse to start for security reasons. This would be the case when
        // running miniserve as a service but forgetting to set the path. This could be pretty
        // dangerous if given with an undesired context path (for instance /root or /).
        if !atty::is(atty::Stream::Stdout) {
            return Err(ContextualError::NoExplicitPathAndNoTerminal);
        }

        warn!("miniserve has been invoked without an explicit path so it will serve the current directory after a short delay.");
        warn!(
            "Invoke with -h|--help to see options or invoke as `miniserve .` to hide this advice."
        );
        print!("Starting server in ");
        io::stdout()
            .flush()
            .map_err(|e| ContextualError::IoError("Failed to write data".to_string(), e))?;
        for c in "3… 2… 1… \n".chars() {
            print!("{}", c);
            io::stdout()
                .flush()
                .map_err(|e| ContextualError::IoError("Failed to write data".to_string(), e))?;
            thread::sleep(Duration::from_millis(500));
        }
    }
    let mut addresses = String::new();
    for interface in &interfaces {
        if !addresses.is_empty() {
            addresses.push_str(", ");
        }
        addresses.push_str(&format!(
            "{}",
            Color::Green
                .paint(format!(
                    "http://{interface}:{port}",
                    interface = &interface,
                    port = miniserve_config.port
                ))
                .bold()
        ));

        if let Some(random_route) = miniserve_config.clone().random_route {
            addresses.push_str(&format!(
                "{}",
                Color::Green
                    .paint(format!("/{random_route}", random_route = random_route,))
                    .bold()
            ));
        }
    }

    let socket_addresses = interfaces
        .iter()
        .map(|interface| {
            format!(
                "{interface}:{port}",
                interface = &interface,
                port = miniserve_config.port,
            )
            .parse::<SocketAddr>()
        })
        .collect::<Result<Vec<SocketAddr>, _>>();

    let socket_addresses = match socket_addresses {
        Ok(addresses) => addresses,
        Err(e) => {
            // Note that this should never fail, since CLI parsing succeeded
            // This means the format of each IP address is valid, and so is the port
            // Valid IpAddr + valid port == valid SocketAddr
            return Err(ContextualError::ParseError(
                "string as socket address".to_string(),
                e.to_string(),
            ));
        }
    };

    let srv = actix_web::HttpServer::new(move || {
        App::new()
            .wrap(configure_header(&inside_config.clone()))
            .app_data(inside_config.clone())
            .wrap_fn(errors::error_page_middleware)
            .wrap(middleware::Condition::new(
                !inside_config.auth.is_empty(),
                HttpAuthentication::basic(auth::handle_auth),
            ))
            .wrap(middleware::Logger::default())
            .route(
                &format!("/{}", inside_config.favicon_route),
                web::get().to(favicon),
            )
            .route(&format!("/{}", inside_config.css_route), web::get().to(css))
            .service(
                web::scope(inside_config.random_route.as_deref().unwrap_or(""))
                    .configure(|c| configure_app(c, &inside_config)),
            )
            .default_service(web::get().to(error_404))
    })
    .bind(socket_addresses.as_slice())
    .map_err(|e| ContextualError::IoError("Failed to bind server".to_string(), e))?
    .shutdown_timeout(0)
    .run();

    println!(
        "Serving path {path} at {addresses}",
        path = Color::Yellow.paint(path_string).bold(),
        addresses = addresses,
    );

    if atty::is(atty::Stream::Stdout) {
        println!("\nQuit by pressing CTRL-C");
    }

    srv.await
        .map_err(|e| ContextualError::IoError("".to_owned(), e))
}

fn configure_header(conf: &MiniserveConfig) -> middleware::DefaultHeaders {
    conf.header.iter().flatten().fold(
        middleware::DefaultHeaders::new(),
        |headers, (header_name, header_value)| headers.header(header_name, header_value),
    )
}

/// Configures the Actix application
fn configure_app(app: &mut web::ServiceConfig, conf: &MiniserveConfig) {
    let static_files_service = || {
        let mut files = actix_files::Files::new("", &conf.path);
        if let Some(ref index_file) = conf.index {
            files = files.index_file(index_file.to_string_lossy());
        }
        if conf.show_hidden {
            files = files.use_hidden_files();
        }
        files
            .show_files_listing()
            .files_listing_renderer(listing::directory_listing)
            .prefer_utf8(true)
            .default_handler(web::to(error_404))
    };
    let file_upload_service =
        || web::resource("/upload").route(web::post().to(file_upload::upload_file));

    if conf.path.is_dir() {
        if conf.file_upload {
            // Allow file upload
            app.service(file_upload_service());
        }
        // Handle directories
        app.service(static_files_service());
    } else {
        // Handle single files
        app.service(web::resource("").route(web::to(listing::file_handler)));
    }
}

async fn error_404(req: HttpRequest) -> Result<HttpResponse, ContextualError> {
    Err(ContextualError::RouteNotFoundError(req.path().to_string()))
}

async fn favicon() -> impl Responder {
    let logo = include_str!("../data/logo.svg");
    web::HttpResponse::Ok()
        .set(ContentType(mime::IMAGE_SVG))
        .message_body(logo.into())
}

async fn css() -> impl Responder {
    let css = include_str!(concat!(env!("OUT_DIR"), "/style.css"));
    web::HttpResponse::Ok()
        .set(ContentType(mime::TEXT_CSS))
        .message_body(css.into())
}
