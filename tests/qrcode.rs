mod fixtures;

use assert_cmd::prelude::*;
use assert_fs::fixture::TempDir;
use fixtures::{port, tmpdir, Error};
use reqwest::StatusCode;
use rstest::rstest;
use select::document::Document;
use select::predicate::Attr;
use std::iter::repeat_with;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

#[rstest]
fn hide_qrcode_element(tmpdir: TempDir, port: u16) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .arg("-p")
        .arg(port.to_string())
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    let body = reqwest::blocking::get(format!("http://localhost:{}", port).as_str())?
        .error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Attr("id", "qrcode")).next().is_none());

    child.kill()?;

    Ok(())
}

#[rstest]
fn show_qrcode_element(tmpdir: TempDir, port: u16) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .arg("-p")
        .arg(port.to_string())
        .arg("-q")
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    let body = reqwest::blocking::get(format!("http://localhost:{}", port).as_str())?
        .error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Attr("id", "qrcode")).next().is_some());

    child.kill()?;

    Ok(())
}

#[rstest]
fn get_svg_qrcode(tmpdir: TempDir, port: u16) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .arg("-p")
        .arg(port.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    // Ok
    let resp = reqwest::blocking::get(format!("http://localhost:{}/?qrcode=test", port).as_str())?;

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers()["Content-Type"], "image/svg+xml");
    let body = resp.text()?;
    assert!(body.starts_with("<?xml"));
    assert_eq!(body.len(), 4450);

    // Err
    let content: String = repeat_with(|| '0').take(8 * 1024).collect();
    let resp =
        reqwest::blocking::get(format!("http://localhost:{}/?qrcode={}", port, content).as_str())?;

    assert_eq!(resp.status(), StatusCode::URI_TOO_LONG);

    child.kill()?;

    Ok(())
}
