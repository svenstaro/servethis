mod fixtures;

use assert_cmd::prelude::*;
use assert_fs::fixture::TempDir;
use fixtures::{port, tmpdir, Error};
use reqwest::StatusCode;
use rstest::rstest;
use select::document::Document;
use select::predicate::Text;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

#[rstest]
fn archives_are_disabled(tmpdir: TempDir, port: u16) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .arg("-p")
        .arg(port.to_string())
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    // Ensure the links to the archives are not present
    let body = reqwest::blocking::get(format!("http://localhost:{}", port).as_str())?
        .error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(parsed
        .find(Text)
        .all(|x| x.text() != "Download .tar.gz" && x.text() != "Download .tar"));

    // Try to download anyway, ensure it's forbidden
    assert_eq!(
        reqwest::blocking::get(format!("http://localhost:{}/?download=tar_gz", port).as_str())?
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        reqwest::blocking::get(format!("http://localhost:{}/?download=tar", port).as_str())?
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        reqwest::blocking::get(format!("http://localhost:{}/?download=zip", port).as_str())?
            .status(),
        StatusCode::FORBIDDEN
    );

    child.kill()?;

    Ok(())
}

#[rstest]
fn test_tar_archives(tmpdir: TempDir, port: u16) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .arg("-p")
        .arg(port.to_string())
        .arg("-g")
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    // Ensure the links to the tar archive exists and tar not exists
    let body = reqwest::blocking::get(format!("http://localhost:{}", port).as_str())?
        .error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Text).any(|x| x.text() == "Download .tar.gz"));
    assert!(parsed.find(Text).all(|x| x.text() != "Download .tar"));

    // Try to download, only tar_gz should works
    assert_eq!(
        reqwest::blocking::get(format!("http://localhost:{}/?download=tar_gz", port).as_str())?
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        reqwest::blocking::get(format!("http://localhost:{}/?download=tar", port).as_str())?
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        reqwest::blocking::get(format!("http://localhost:{}/?download=zip", port).as_str())?
            .status(),
        StatusCode::FORBIDDEN
    );

    child.kill()?;

    Ok(())
}
