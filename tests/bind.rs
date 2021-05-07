mod fixtures;

use assert_cmd::prelude::*;
use assert_fs::fixture::TempDir;
use fixtures::{port, tmpdir, Error};
use regex::Regex;
use rstest::rstest;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

#[rstest(
    args,
    case(&["-i", "012.123.234.012"]),
    case(&["-i", "::", "-i", "012.123.234.012"]),
)]
fn bind_fails(tmpdir: TempDir, port: u16, args: &[&str]) -> Result<(), Error> {
    Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .arg("-p")
        .arg(port.to_string())
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .assert()
        .failure();

    Ok(())
}

#[rstest(
    args, bind_ipv4, bind_ipv6,
    case(&[] as &[&str], true, true),
    case(&["-i", "::"], false, true),
    case(&["-i", "0.0.0.0"], true, false),
    case(&["-i", "::", "-i", "0.0.0.0"], true, true),
)]
fn bind_ipv4_ipv6(
    tmpdir: TempDir,
    port: u16,
    args: &[&str],
    bind_ipv4: bool,
    bind_ipv6: bool,
) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .arg("-p")
        .arg(port.to_string())
        .args(args)
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    assert_eq!(
        reqwest::blocking::get(format!("http://127.0.0.1:{}", port).as_str()).is_ok(),
        bind_ipv4
    );
    assert_eq!(
        reqwest::blocking::get(format!("http://[::1]:{}", port).as_str()).is_ok(),
        bind_ipv6
    );

    child.kill()?;

    Ok(())
}

#[rstest(
    args,
    case(&[] as &[&str]),
    case(&["-i", "::"]),
    case(&["-i", "127.0.0.1"]),
    case(&["-i", "0.0.0.0"]),
    case(&["-i", "::", "-i", "0.0.0.0"]),
    case(&["--random-route"]),
)]
fn validate_prtinted_urls(tmpdir: TempDir, port: u16, args: &[&str]) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .arg("-p")
        .arg(port.to_string())
        .args(args)
        .stdout(Stdio::piped())
        .spawn()?;

    sleep(Duration::from_secs(1));

    let urls_line = BufReader::new(child.stdout.take().unwrap())
        .lines()
        .map(|line| line.expect("Error reading stdout"))
        .filter(|line| line.starts_with("Serving path"))
        .nth(0)
        .unwrap();

    let urls = Regex::new(r"http://[a-zA-Z0-9\.\[\]:/]+")
        .unwrap()
        .captures_iter(urls_line.as_str())
        .map(|caps| caps.get(0).unwrap().as_str())
        .collect::<Vec<_>>();

    assert!(!urls.is_empty());

    for url in urls {
        reqwest::blocking::get(url)?.error_for_status()?;
    }

    child.kill()?;

    Ok(())
}
