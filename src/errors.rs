use thiserror::Error;

#[derive(Debug, Error)]
pub enum ContextualError {
    /// Any kind of IO errors
    #[error("{0}\ncaused by: {1}")]
    IoError(String, std::io::Error),

    /// Might occur during file upload, when processing the multipart request fails
    #[error("Failed to process multipart request\ncaused by: {0}")]
    MultipartError(actix_multipart::MultipartError),

    /// Might occur during file upload
    #[error("File already exists, and the overwrite_files option has not been set")]
    DuplicateFileError,

    /// Might occur during mkdir
    #[error("Directory or file already exist")]
    ConflictMkdirError,

    /// Any error related to an invalid path (failed to retrieve entry name, unexpected entry type, etc)
    #[error("Invalid path\ncaused by: {0}")]
    InvalidPathError(String),

    /// Might occur if the HTTP credential string does not respect the expected format
    #[error("Invalid format for credentials string. Expected username:password, username:sha256:hash or username:sha512:hash")]
    InvalidAuthFormat,

    /// Might occure if the hash method is neither sha256 nor sha512
    #[error("{0} is not a valid hashing method. Expected sha256 or sha512")]
    InvalidHashMethod(String),

    /// Might occur if the HTTP auth hash password is not a valid hex code
    #[error("Invalid format for password hash. Expected hex code")]
    InvalidPasswordHash,

    /// Might occur if the HTTP auth password exceeds 255 characters
    #[error("HTTP password length exceeds 255 characters")]
    PasswordTooLongError,

    /// Might occur if the user has unsufficient permissions to create an entry in a given directory
    #[error("Insufficient permissions to create file in {0}")]
    InsufficientPermissionsError(String),

    /// Any error related to parsing
    #[error("Failed to parse {0}\ncaused by: {1}")]
    ParseError(String, String),

    /// Might occur when the creation of an archive fails
    #[error("An error occured while creating the {0}\ncaused by: {1}")]
    ArchiveCreationError(String, Box<ContextualError>),

    /// More specific archive creation failure reason
    #[error("{0}")]
    ArchiveCreationDetailError(String),

    /// Might occur when the HTTP authentication fails
    #[error("An error occured during HTTP authentication\ncaused by: {0}")]
    HttpAuthenticationError(Box<ContextualError>),

    /// Might occur when the HTTP credentials are not correct
    #[error("Invalid credentials for HTTP authentication")]
    InvalidHttpCredentials,

    /// Might occur when an HTTP request is invalid
    #[error("Invalid HTTP request\ncaused by: {0}")]
    InvalidHttpRequestError(String),

    /// Might occur when trying to access a page that does not exist
    #[error("Route {0} could not be found")]
    RouteNotFoundError(String),

    /// In case miniserve was invoked without an interactive terminal and without an explicit path
    #[error("Refusing to start as no explicit serve path was set and no interactive terminal was attached
Please set an explicit serve path like: `miniserve /my/path`")]
    NoExplicitPathAndNoTerminal,

    /// In case miniserve was invoked with --no-symlinks but the serve path is a symlink
    #[error("The -P|--no-symlinks option was provided but the serve path '{0}' is a symlink")]
    NoSymlinksOptionWithSymlinkServePath(String),
}

pub fn log_error_chain(description: String) {
    for cause in description.lines() {
        log::error!("{}", cause);
    }
}
