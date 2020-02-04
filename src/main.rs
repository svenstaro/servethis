#![feature(proc_macro_hygiene)]

use actix_web::http::{Method, StatusCode};
use actix_web::{fs, middleware, server, App, HttpRequest, HttpResponse};
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::thread;
use std::time::Duration;
use structopt::clap::crate_version;
use yansi::{Color, Paint};

mod archive;
mod args;
mod auth;
mod errors;
mod file_upload;
mod listing;
mod pipe;
mod renderer;
mod themes;

use crate::errors::ContextualError;

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

    /// Enable URL path prefix
    pub path_prefix: Option<String>,

    /// Default color scheme
    pub default_color_scheme: themes::ColorScheme,

    /// Enable file upload
    pub file_upload: bool,

    /// Enable upload to override existing files
    pub overwrite_files: bool,
}

fn main() {
    match run() {
        Ok(()) => (),
        Err(e) => errors::log_error_chain(e.to_string()),
    }
}

fn run() -> Result<(), ContextualError> {
    if cfg!(windows) && !Paint::enable_windows_ascii() {
        Paint::disable();
    }

    let sys = actix::System::new("miniserve");
    let miniserve_config = args::parse_args();

    let log_level = if miniserve_config.verbose {
        simplelog::LevelFilter::Info
    } else {
        simplelog::LevelFilter::Error
    };

    if simplelog::TermLogger::init(
        log_level,
        simplelog::Config::default(),
        simplelog::TerminalMode::Mixed,
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
                ContextualError::IOError("Failed to retrieve symlink's metadata".to_string(), e)
            })?
            .file_type()
            .is_symlink();

        if is_symlink {
            return Err(ContextualError::from(
                "The no-symlinks option cannot be used with a symlink path".to_string(),
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
        ContextualError::IOError("Failed to resolve path to be served".to_string(), e)
    })?;
    let path_string = canon_path.to_string_lossy();

    println!(
        "{name} v{version}",
        name = Paint::new("miniserve").bold(),
        version = crate_version!()
    );
    if !miniserve_config.path_explicitly_chosen {
        println!("{warning} miniserve has been invoked without an explicit path so it will serve the current directory.", warning=Color::RGB(255, 192, 0).paint("Notice:").bold());
        println!(
            "      Invoke with -h|--help to see options or invoke as `miniserve .` to hide this advice."
        );
        print!("Starting server in ");
        io::stdout()
            .flush()
            .map_err(|e| ContextualError::IOError("Failed to write data".to_string(), e))?;
        for c in "3… 2… 1… \n".chars() {
            print!("{}", c);
            io::stdout()
                .flush()
                .map_err(|e| ContextualError::IOError("Failed to write data".to_string(), e))?;
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

        if let Some(path_prefix) = miniserve_config.clone().path_prefix {
            addresses.push_str(&format!(
                "{}",
                Color::Green.paint(format!("/{}", path_prefix)).bold()
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

    server::new(move || {
        App::with_state(inside_config.clone())
            .middleware(auth::Auth)
            .middleware(middleware::Logger::default())
            .configure(configure_app)
    })
    .bind(socket_addresses.as_slice())
    .map_err(|e| ContextualError::IOError("Failed to bind server".to_string(), e))?
    .shutdown_timeout(0)
    .start();

    println!(
        "Serving path {path} at {addresses}",
        path = Color::Yellow.paint(path_string).bold(),
        addresses = addresses,
    );

    println!("\nQuit by pressing CTRL-C");

    let _ = sys.run();

    Ok(())
}

/// Configures the Actix application
fn configure_app(app: App<MiniserveConfig>) -> App<MiniserveConfig> {
    let upload_route;
    let s = {
        let path = &app.state().path;
        let no_symlinks = app.state().no_symlinks;
        let path_prefix = app.state().path_prefix.clone();
        let default_color_scheme = app.state().default_color_scheme;
        let file_upload = app.state().file_upload;
        upload_route = if let Some(path_prefix) = app.state().path_prefix.clone() {
            format!("/{}/upload", path_prefix)
        } else {
            "/upload".to_string()
        };
        if path.is_file() {
            None
        } else {
            let u_r = upload_route.clone();
            Some(
                fs::StaticFiles::new(path)
                    .expect("Failed to setup static file handler")
                    .show_files_listing()
                    .files_listing_renderer(move |dir, req| {
                        listing::directory_listing(
                            dir,
                            req,
                            no_symlinks,
                            file_upload,
                            path_prefix.clone(),
                            default_color_scheme,
                            u_r.clone(),
                        )
                    })
                    .default_handler(error_404),
            )
        }
    };

    let path_prefix = app.state().path_prefix.clone().unwrap_or_default();
    let full_route = format!("/{}", path_prefix);

    if let Some(s) = s {
        if app.state().file_upload {
            let default_color_scheme = app.state().default_color_scheme;
            // Allow file upload
            app.resource(&upload_route, move |r| {
                r.method(Method::POST)
                    .f(move |file| file_upload::upload_file(file, default_color_scheme))
            })
            // Handle directories
            .handler(&full_route, s)
            .default_resource(|r| r.method(Method::GET).f(error_404))
        } else {
            // Handle directories
            app.handler(&full_route, s)
                .default_resource(|r| r.method(Method::GET).f(error_404))
        }
    } else {
        // Handle single files
        app.resource(&full_route, |r| r.f(listing::file_handler))
            .default_resource(|r| r.method(Method::GET).f(error_404))
    }
}

fn error_404(req: &HttpRequest<crate::MiniserveConfig>) -> Result<HttpResponse, io::Error> {
    let err_404 = ContextualError::RouteNotFoundError(req.path().to_string());
    let default_color_scheme = req.state().default_color_scheme;
    let return_address = match &req.state().path_prefix {
        Some(path_prefix) => format!("/{}", path_prefix),
        None => "/".to_string(),
    };

    let query_params = listing::extract_query_parameters(req);
    let color_scheme = query_params.theme.unwrap_or(default_color_scheme);

    errors::log_error_chain(err_404.to_string());

    Ok(actix_web::HttpResponse::NotFound().body(
        renderer::render_error(
            &err_404.to_string(),
            StatusCode::NOT_FOUND,
            &return_address,
            query_params.sort,
            query_params.order,
            color_scheme,
            default_color_scheme,
            false,
            true,
        )
        .into_string(),
    ))
}
