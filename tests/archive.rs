mod fixtures;

use fixtures::{server, Error, TestServer};
use reqwest::StatusCode;
use rstest::rstest;
use select::document::Document;
use select::predicate::Text;

#[rstest]
fn archives_are_disabled(server: TestServer) -> Result<(), Error> {
    // Ensure the links to the archives are not present
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(parsed
        .find(Text)
        .all(|x| x.text() != "Download .tar.gz" && x.text() != "Download .tar"));

    // Try to download anyway, ensure it's forbidden
    assert_eq!(
        reqwest::blocking::get(server.url().join("?download=tar_gz")?)?.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        reqwest::blocking::get(server.url().join("?download=tar")?)?.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        reqwest::blocking::get(server.url().join("?download=zip")?)?.status(),
        StatusCode::FORBIDDEN
    );

    Ok(())
}

#[rstest(server(&["-g"]))]
fn test_tar_archives(server: TestServer) -> Result<(), Error> {
    // Ensure the links to the tar archive exists and tar not exists
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Text).any(|x| x.text() == "Download .tar.gz"));
    assert!(parsed.find(Text).all(|x| x.text() != "Download .tar"));

    // Try to download, only tar_gz should works
    assert_eq!(
        reqwest::blocking::get(server.url().join("?download=tar_gz")?)?.status(),
        StatusCode::OK
    );
    assert_eq!(
        reqwest::blocking::get(server.url().join("?download=tar")?)?.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        reqwest::blocking::get(server.url().join("?download=zip")?)?.status(),
        StatusCode::FORBIDDEN
    );

    Ok(())
}
