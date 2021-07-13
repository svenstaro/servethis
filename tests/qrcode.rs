mod fixtures;

use fixtures::{server, server_no_stderr, Error, TestServer};
use reqwest::StatusCode;
use rstest::rstest;
use select::document::Document;
use select::predicate::Attr;
use std::iter::repeat_with;

#[rstest]
fn hide_qrcode_element(server: TestServer) -> Result<(), Error> {
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Attr("id", "qrcode")).next().is_none());

    Ok(())
}

#[rstest(server(&["-q"]))]
fn show_qrcode_element(server: TestServer) -> Result<(), Error> {
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Attr("id", "qrcode")).next().is_some());

    Ok(())
}

#[rstest]
fn get_svg_qrcode(server_no_stderr: TestServer) -> Result<(), Error> {
    let server = server_no_stderr;
    // Ok
    let resp = reqwest::blocking::get(server.url().join("/?qrcode=test")?)?;

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers()["Content-Type"], "image/svg+xml");
    let body = resp.text()?;
    assert!(body.starts_with("<?xml"));
    assert_eq!(body.len(), 3530);

    // Err
    let content: String = repeat_with(|| '0').take(8 * 1024).collect();
    let resp = reqwest::blocking::get(server.url().join(&format!("?qrcode={}", content))?)?;

    assert_eq!(resp.status(), StatusCode::URI_TOO_LONG);

    Ok(())
}
