use actix_web::http::{header, StatusCode};
use actix_web::middleware::{Middleware, Response};
use actix_web::{HttpRequest, HttpResponse, Result};
use sha2::{Digest, Sha256, Sha512};

use crate::errors::{self, ContextualError};
use crate::renderer;

pub struct Auth;

#[derive(Clone, Debug)]
/// HTTP Basic authentication parameters
pub struct BasicAuthParams {
    pub username: String,
    pub password: String,
}

#[derive(Clone, Debug, PartialEq)]
/// `password` field of `RequiredAuth`
pub enum RequiredAuthPassword {
    Plain(String),
    Sha256(Vec<u8>),
    Sha512(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq)]
/// Authentication structure to match `BasicAuthParams` against
pub struct RequiredAuth {
    pub username: String,
    pub password: RequiredAuthPassword,
}

/// Decode a HTTP basic auth string into a tuple of username and password.
pub fn parse_basic_auth(
    authorization_header: &header::HeaderValue,
) -> Result<BasicAuthParams, ContextualError> {
    let basic_removed = authorization_header
        .to_str()
        .map_err(|e| {
            ContextualError::ParseError("HTTP authentication header".to_string(), e.to_string())
        })?
        .replace("Basic ", "");
    let decoded = base64::decode(&basic_removed).map_err(ContextualError::Base64DecodeError)?;
    let decoded_str = String::from_utf8_lossy(&decoded);
    let credentials: Vec<&str> = decoded_str.splitn(2, ':').collect();

    // If argument parsing went fine, it means the HTTP credentials string is well formatted
    // So we can safely unpack the username and the password

    Ok(BasicAuthParams {
        username: credentials[0].to_owned(),
        password: credentials[1].to_owned(),
    })
}

/// Return `true` if `basic_auth` is matches any of `required_auth`
pub fn match_auth(basic_auth: BasicAuthParams, required_auth: &[RequiredAuth]) -> bool {
    required_auth
        .iter()
        .any(|RequiredAuth { username, password }| {
            basic_auth.username == *username && compare_password(&basic_auth.password, password)
        })
}

/// Return `true` if `basic_auth_pwd` meets `required_auth_pwd`'s requirement
pub fn compare_password(basic_auth_pwd: &str, required_auth_pwd: &RequiredAuthPassword) -> bool {
    match &required_auth_pwd {
        RequiredAuthPassword::Plain(required_password) => *basic_auth_pwd == *required_password,
        RequiredAuthPassword::Sha256(password_hash) => {
            compare_hash::<Sha256>(basic_auth_pwd, password_hash)
        }
        RequiredAuthPassword::Sha512(password_hash) => {
            compare_hash::<Sha512>(basic_auth_pwd, password_hash)
        }
    }
}

/// Return `true` if hashing of `password` by `T` algorithm equals to `hash`
pub fn compare_hash<T: Digest>(password: &str, hash: &[u8]) -> bool {
    get_hash::<T>(password) == hash
}

/// Get hash of a `text`
pub fn get_hash<T: Digest>(text: &str) -> Vec<u8> {
    let mut hasher = T::new();
    hasher.input(text);
    hasher.result().to_vec()
}

impl Middleware<crate::MiniserveConfig> for Auth {
    fn response(
        &self,
        req: &HttpRequest<crate::MiniserveConfig>,
        resp: HttpResponse,
    ) -> Result<Response> {
        let required_auth = &req.state().auth;

        if required_auth.is_empty() {
            return Ok(Response::Done(resp));
        }

        if let Some(auth_headers) = req.headers().get(header::AUTHORIZATION) {
            let auth_req = match parse_basic_auth(auth_headers) {
                Ok(auth_req) => auth_req,
                Err(err) => {
                    let auth_err = ContextualError::HTTPAuthenticationError(Box::new(err));
                    return Ok(Response::Done(HttpResponse::BadRequest().body(
                        build_unauthorized_response(&req, auth_err, true, StatusCode::BAD_REQUEST),
                    )));
                }
            };

            if match_auth(auth_req, required_auth) {
                return Ok(Response::Done(resp));
            }
        }

        Ok(Response::Done(
            HttpResponse::Unauthorized()
                .header(
                    header::WWW_AUTHENTICATE,
                    header::HeaderValue::from_static("Basic realm=\"miniserve\""),
                )
                .body(build_unauthorized_response(
                    &req,
                    ContextualError::InvalidHTTPCredentials,
                    true,
                    StatusCode::UNAUTHORIZED,
                )),
        ))
    }
}

/// Builds the unauthorized response body
/// The reason why log_error_chain is optional is to handle cases where the auth pop-up appears and when the user clicks Cancel.
/// In those case, we do not log the error to the terminal since it does not really matter.
fn build_unauthorized_response(
    req: &HttpRequest<crate::MiniserveConfig>,
    error: ContextualError,
    log_error_chain: bool,
    error_code: StatusCode,
) -> String {
    let error = ContextualError::HTTPAuthenticationError(Box::new(error));

    if log_error_chain {
        errors::log_error_chain(error.to_string());
    }
    let return_path = match &req.state().path_prefix {
        Some(path_prefix) => format!("/{}", path_prefix),
        None => "/".to_string(),
    };

    renderer::render_error(
        &error.to_string(),
        error_code,
        &return_path,
        None,
        None,
        req.state().default_color_scheme,
        req.state().default_color_scheme,
        false,
        false,
    )
    .into_string()
}

#[rustfmt::skip]
#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{rstest, rstest_parametrize, fixture};
    use pretty_assertions::assert_eq;

    /// Return a hashing function corresponds to given name
    fn get_hash_func(name: &str) -> impl FnOnce(&str) -> Vec<u8> {
        match name {
            "sha256" => get_hash::<Sha256>,
            "sha512" => get_hash::<Sha512>,
            _ => panic!("Invalid hash method"),
        }
    }

    #[rstest_parametrize(
        password, hash_method, hash,
        case("abc", "sha256", "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"),
        case("abc", "sha512", "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"),
    )]
    fn test_get_hash(password: &str, hash_method: &str, hash: &str) {
        let hash_func = get_hash_func(hash_method);
        let expected = hex::decode(hash).expect("Provided hash is not a valid hex code");
        let received = hash_func(&password.to_owned());
        assert_eq!(received, expected);
    }

    /// Helper function that creates a `RequiredAuth` structure and encrypt `password` if necessary
    fn create_required_auth(username: &str, password: &str, encrypt: &str) -> RequiredAuth {
        use RequiredAuthPassword::*;

        let password = match encrypt {
            "plain" => Plain(password.to_owned()),
            "sha256" => Sha256(get_hash::<sha2::Sha256>(&password.to_owned())),
            "sha512" => Sha512(get_hash::<sha2::Sha512>(&password.to_owned())),
            _ => panic!("Unknown encryption type"),
        };

        RequiredAuth {
            username: username.to_owned(),
            password,
        }
    }

    #[rstest_parametrize(
        should_pass, param_username, param_password, required_username, required_password, encrypt,
        case(true, "obi", "hello there", "obi", "hello there", "plain"),
        case(false, "obi", "hello there", "obi", "hi!", "plain"),
        case(true, "obi", "hello there", "obi", "hello there", "sha256"),
        case(false, "obi", "hello there", "obi", "hi!", "sha256"),
        case(true, "obi", "hello there", "obi", "hello there", "sha512"),
        case(false, "obi", "hello there", "obi", "hi!", "sha512")
    )]
    fn test_single_auth(
        should_pass: bool,
        param_username: &str,
        param_password: &str,
        required_username: &str,
        required_password: &str,
        encrypt: &str,
    ) {
        assert_eq!(
            match_auth(
                BasicAuthParams {
                    username: param_username.to_owned(),
                    password: param_password.to_owned(),
                },
                &[create_required_auth(required_username, required_password, encrypt)],
            ),
            should_pass,
        )
    }

    /// Helper function that creates a sample of multiple accounts
    #[fixture]
    fn account_sample() -> Vec<RequiredAuth> {
        [
            ("usr0", "pwd0", "plain"),
            ("usr1", "pwd1", "plain"),
            ("usr2", "pwd2", "sha256"),
            ("usr3", "pwd3", "sha256"),
            ("usr4", "pwd4", "sha512"),
            ("usr5", "pwd5", "sha512"),
        ]
            .iter()
            .map(|(username, password, encrypt)| create_required_auth(username, password, encrypt))
            .collect()
    }

    #[rstest_parametrize(
        username, password,
        case("usr0", "pwd0"),
        case("usr1", "pwd1"),
        case("usr2", "pwd2"),
        case("usr3", "pwd3"),
        case("usr4", "pwd4"),
        case("usr5", "pwd5"),
    )]
    fn test_multiple_auth_pass(
        account_sample: Vec<RequiredAuth>,
        username: &str,
        password: &str,
    ) {
        assert!(match_auth(
            BasicAuthParams {
                username: username.to_owned(),
                password: password.to_owned(),
            },
            &account_sample,
        ));
    }

    #[rstest]
    fn test_multiple_auth_wrong_username(account_sample: Vec<RequiredAuth>) {
        assert_eq!(match_auth(
            BasicAuthParams {
                username: "unregistered user".to_owned(),
                password: "pwd0".to_owned(),
            },
            &account_sample,
        ), false);
    }

    #[rstest_parametrize(
        username, password,
        case("usr0", "pwd5"),
        case("usr1", "pwd4"),
        case("usr2", "pwd3"),
        case("usr3", "pwd2"),
        case("usr4", "pwd1"),
        case("usr5", "pwd0"),
    )]
    fn test_multiple_auth_wrong_password(
        account_sample: Vec<RequiredAuth>,
        username: &str,
        password: &str,
    ) {
        assert_eq!(match_auth(
            BasicAuthParams {
                username: username.to_owned(),
                password: password.to_owned(),
            },
            &account_sample,
        ), false);
    }
}
