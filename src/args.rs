use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;
use structopt::StructOpt;

use crate::auth;
use crate::errors::ContextualError;
use crate::themes;

/// Possible characters for random routes
const ROUTE_ALPHABET: [char; 16] = [
    '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', 'a', 'b', 'c', 'd', 'e', 'f',
];

#[derive(StructOpt)]
#[structopt(
    name = "miniserve",
    author,
    about,
    global_settings = &[structopt::clap::AppSettings::ColoredHelp],
)]
struct CLIArgs {
    /// Be verbose, includes emitting access logs
    #[structopt(short = "v", long = "verbose")]
    verbose: bool,

    /// Which path to serve
    #[structopt(name = "PATH", parse(from_os_str))]
    path: Option<PathBuf>,

    /// Port to use
    #[structopt(short = "p", long = "port", default_value = "8080")]
    port: u16,

    /// Interface to listen on
    #[structopt(
        short = "i",
        long = "interfaces",
        parse(try_from_str = parse_interface),
        number_of_values = 1,
    )]
    interfaces: Vec<IpAddr>,

    /// Set authentication. Currently supported formats:
    /// username:password, username:sha256:hash, username:sha512:hash
    /// (e.g. joe:123, joe:sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3)
    #[structopt(
        short = "a",
        long = "auth",
        parse(try_from_str = parse_auth),
        number_of_values = 1,
    )]
    auth: Vec<auth::RequiredAuth>,

    /// Use a specific path prefix
    #[structopt(long = "path-prefix")]
    path_prefix: Option<String>,

    /// Use a random path prefix
    #[structopt(long = "random-path-prefix", conflicts_with("path-prefix"))]
    random_path_prefix: bool,

    /// Do not follow symbolic links
    #[structopt(short = "P", long = "no-symlinks")]
    no_symlinks: bool,

    /// Default color scheme
    #[structopt(
        short = "c",
        long = "color-scheme",
        default_value = "Squirrel",
        possible_values = &themes::ColorScheme::variants(),
        case_insensitive = true,
    )]
    color_scheme: themes::ColorScheme,

    /// Enable file uploading
    #[structopt(short = "u", long = "upload-files")]
    file_upload: bool,

    /// Enable overriding existing files during file upload
    #[structopt(short = "o", long = "overwrite-files")]
    overwrite_files: bool,
}

/// Checks wether an interface is valid, i.e. it can be parsed into an IP address
fn parse_interface(src: &str) -> Result<IpAddr, std::net::AddrParseError> {
    src.parse::<IpAddr>()
}

/// Parse authentication requirement
fn parse_auth(src: &str) -> Result<auth::RequiredAuth, ContextualError> {
    let mut split = src.splitn(3, ':');
    let invalid_auth_format = Err(ContextualError::InvalidAuthFormat);

    let username = match split.next() {
        Some(username) => username,
        None => return invalid_auth_format,
    };

    // second_part is either password in username:password or method in username:method:hash
    let second_part = match split.next() {
        // This allows empty passwords, as the spec does not forbid it
        Some(password) => password,
        None => return invalid_auth_format,
    };

    let password = if let Some(hash_hex) = split.next() {
        let hash_bin = if let Ok(hash_bin) = hex::decode(hash_hex) {
            hash_bin
        } else {
            return Err(ContextualError::InvalidPasswordHash);
        };

        match second_part {
            "sha256" => auth::RequiredAuthPassword::Sha256(hash_bin.to_owned()),
            "sha512" => auth::RequiredAuthPassword::Sha512(hash_bin.to_owned()),
            _ => return Err(ContextualError::InvalidHashMethod(second_part.to_owned())),
        }
    } else {
        // To make it Windows-compatible, the password needs to be shorter than 255 characters.
        // After 255 characters, Windows will truncate the value.
        // As for the username, the spec does not mention a limit in length
        if second_part.len() > 255 {
            return Err(ContextualError::PasswordTooLongError);
        }

        auth::RequiredAuthPassword::Plain(second_part.to_owned())
    };

    Ok(auth::RequiredAuth {
        username: username.to_owned(),
        password,
    })
}

/// Parses the command line arguments
pub fn parse_args() -> crate::MiniserveConfig {
    let args = CLIArgs::from_args();

    let interfaces = if !args.interfaces.is_empty() {
        args.interfaces
    } else {
        vec![
            IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)),
            IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
        ]
    };

    let path_prefix = if args.random_path_prefix {
        Some(nanoid::custom(6, &ROUTE_ALPHABET))
    } else {
        args.path_prefix
    };

    let default_color_scheme = args.color_scheme;

    let path_explicitly_chosen = args.path.is_some();

    crate::MiniserveConfig {
        verbose: args.verbose,
        path: args.path.unwrap_or_else(|| PathBuf::from(".")),
        port: args.port,
        interfaces,
        auth: args.auth,
        path_explicitly_chosen,
        no_symlinks: args.no_symlinks,
        path_prefix,
        default_color_scheme,
        overwrite_files: args.overwrite_files,
        file_upload: args.file_upload,
    }
}

#[rustfmt::skip]
#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest_parametrize;
    use pretty_assertions::assert_eq;

    /// Helper function that creates a `RequiredAuth` structure
    fn create_required_auth(username: &str, password: &str, encrypt: &str) -> auth::RequiredAuth {
        use auth::*;
        use RequiredAuthPassword::*;

        let password = match encrypt {
            "plain" => Plain(password.to_owned()),
            "sha256" => Sha256(hex::decode(password.to_owned()).unwrap()),
            "sha512" => Sha512(hex::decode(password.to_owned()).unwrap()),
            _ => panic!("Unknown encryption type"),
        };

        auth::RequiredAuth {
            username: username.to_owned(),
            password,
        }
    }

    #[rstest_parametrize(
        auth_string, username, password, encrypt,
        case("username:password", "username", "password", "plain"),
        case("username:sha256:abcd", "username", "abcd", "sha256"),
        case("username:sha512:abcd", "username", "abcd", "sha512")
    )]
    fn parse_auth_valid(auth_string: &str, username: &str, password: &str, encrypt: &str) {
        assert_eq!(
            parse_auth(auth_string).unwrap(),
            create_required_auth(username, password, encrypt),
        );
    }

    #[rstest_parametrize(
        auth_string, err_msg,
        case(
            "foo",
            "Invalid format for credentials string. Expected username:password, username:sha256:hash or username:sha512:hash"
        ),
        case(
            "username:blahblah:abcd",
            "blahblah is not a valid hashing method. Expected sha256 or sha512"
        ),
        case(
            "username:sha256:invalid",
            "Invalid format for password hash. Expected hex code"
        ),
        case(
            "username:sha512:invalid",
            "Invalid format for password hash. Expected hex code"
        ),
    )]
    fn parse_auth_invalid(auth_string: &str, err_msg: &str) {
        let err = parse_auth(auth_string).unwrap_err();
        assert_eq!(format!("{}", err), err_msg.to_owned());
    }
}
