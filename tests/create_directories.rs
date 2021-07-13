mod fixtures;

use assert_cmd::prelude::*;
use assert_fs::fixture::TempDir;
use fixtures::{port, tmpdir, Error};
use reqwest::blocking::Client;
use rstest::rstest;
use select::document::Document;
use select::predicate::{Attr, Text};
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

#[rstest]
fn create_directories_works(tmpdir: TempDir, port: u16) -> Result<(), Error> {
    let test_dir_name = "create_test_dir";

    let mut child = Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .arg("-p")
        .arg(port.to_string())
        .arg("-m")
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    // Before creating, check whether the created directory does not yet exist.
    let body = reqwest::blocking::get(format!("http://localhost:{}", port).as_str())?
        .error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(!parsed
        .find(Text)
        .any(|x| x.text() == format!("{}/", test_dir_name)));

    // Perform the actual upload.
    let mkdir_action = parsed
        .find(Attr("id", "mkdir"))
        .next()
        .expect("Couldn't find element with id=mkdir")
        .attr("action")
        .expect("Upload form doesn't have action attribute");

    let client = Client::new();
    client
        .post(format!("http://localhost:{}{}", port, mkdir_action).as_str())
        .form(&[("mkdir_name", test_dir_name)])
        .send()?
        .error_for_status()?;

    // After creating, check whether the created directory is now getting listed.
    let body = reqwest::blocking::get(format!("http://localhost:{}", port).as_str())?;
    let parsed = Document::from_read(body)?;
    assert!(parsed
        .find(Text)
        .any(|x| x.text() == format!("{}/", test_dir_name)));

    child.kill()?;

    Ok(())
}

#[rstest]
fn creating_directories_is_prevented(tmpdir: TempDir, port: u16) -> Result<(), Error> {
    let test_dir_name = "create_test_dir";

    let mut child = Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .arg("-p")
        .arg(port.to_string())
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    // Before creating, check whether the created directory does not yet exist.
    let body = reqwest::blocking::get(format!("http://localhost:{}", port).as_str())?
        .error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(!parsed
        .find(Text)
        .any(|x| x.text() == format!("{}/", test_dir_name)));

    // Ensure the create directory form is not present
    assert!(parsed.find(Attr("id", "mkdir")).next().is_none());

    // Then try to create directory anyway
    let client = Client::new();
    // Ensure creating fails and returns an error
    assert!(client
        .post(format!("http://localhost:{}{}", port, "/mkdir?path=/").as_str())
        .form(&[("mkdir_name", test_dir_name)])
        .send()?
        .error_for_status()
        .is_err());

    // After creating, check whether the created directory is now getting listed.
    let body = reqwest::blocking::get(format!("http://localhost:{}", port).as_str())?;
    let parsed = Document::from_read(body)?;
    assert!(!parsed
        .find(Text)
        .any(|x| x.text() == format!("{}/", test_dir_name)));

    child.kill()?;

    Ok(())
}
