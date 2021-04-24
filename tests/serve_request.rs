mod fixtures;

use assert_cmd::prelude::*;
use assert_fs::fixture::TempDir;
use fixtures::{
    port, server, tmpdir, Error, TestServer, DIRECTORIES, FILES, HIDDEN_DIRECTORIES, HIDDEN_FILES,
};
use http::StatusCode;
use regex::Regex;
use rstest::rstest;
use select::document::Document;
use select::node::Node;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::fs::{symlink as symlink_dir, symlink as symlink_file};
#[cfg(windows)]
use std::os::windows::fs::{symlink_dir, symlink_file};

#[rstest]
fn serves_requests_with_no_options(tmpdir: TempDir) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    let body = reqwest::blocking::get("http://localhost:8080")?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    for &file in FILES {
        assert!(parsed.find(|x: &Node| x.text() == file).next().is_some());
    }

    child.kill()?;

    Ok(())
}

#[rstest]
fn serves_requests_with_non_default_port(server: TestServer) -> Result<(), Error> {
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;

    for &file in FILES {
        let f = parsed.find(|x: &Node| x.text() == file).next().unwrap();
        reqwest::blocking::get(server.url().join(f.attr("href").unwrap())?)?.error_for_status()?;
        assert_eq!(
            format!("/{}", file),
            percent_encoding::percent_decode_str(f.attr("href").unwrap()).decode_utf8_lossy(),
        );
    }

    for &directory in DIRECTORIES {
        assert!(parsed
            .find(|x: &Node| x.text() == directory)
            .next()
            .is_some());
        let dir_body =
            reqwest::blocking::get(server.url().join(&directory)?)?.error_for_status()?;
        let dir_body_parsed = Document::from_read(dir_body)?;
        for &file in FILES {
            assert!(dir_body_parsed
                .find(|x: &Node| x.text() == file)
                .next()
                .is_some());
        }
    }

    Ok(())
}

#[rstest(server(&["--hidden"]))]
fn serves_requests_hidden_files(server: TestServer) -> Result<(), Error> {
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;

    for &file in FILES.into_iter().chain(HIDDEN_FILES) {
        let f = parsed.find(|x: &Node| x.text() == file).next().unwrap();
        assert_eq!(
            format!("/{}", file),
            percent_encoding::percent_decode_str(f.attr("href").unwrap()).decode_utf8_lossy(),
        );
    }

    for &directory in DIRECTORIES.into_iter().chain(HIDDEN_DIRECTORIES) {
        assert!(parsed
            .find(|x: &Node| x.text() == directory)
            .next()
            .is_some());
        let dir_body =
            reqwest::blocking::get(server.url().join(&directory)?)?.error_for_status()?;
        let dir_body_parsed = Document::from_read(dir_body)?;
        for &file in FILES.into_iter().chain(HIDDEN_FILES) {
            assert!(dir_body_parsed
                .find(|x: &Node| x.text() == file)
                .next()
                .is_some());
        }
    }

    Ok(())
}

#[rstest]
fn serves_requests_no_hidden_files_without_flag(server: TestServer) -> Result<(), Error> {
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;

    for &hidden_item in HIDDEN_FILES.into_iter().chain(HIDDEN_DIRECTORIES) {
        assert!(parsed
            .find(|x: &Node| x.text() == hidden_item)
            .next()
            .is_none());
        let resp = reqwest::blocking::get(server.url().join(&hidden_item)?)?;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    Ok(())
}

#[rstest(no_symlinks, case(true), case(false))]
fn serves_requests_symlinks(no_symlinks: bool) -> Result<(), Error> {
    let server = server(["--no-symlinks"].iter().filter(|_| no_symlinks));
    let files = &["symlink-file.html"];
    let dirs = &["symlink-dir/"];
    let broken = &["symlink broken"];

    for &directory in dirs {
        let orig = DIRECTORIES[0].strip_suffix("/").unwrap();
        let link = server.path().join(directory.strip_suffix("/").unwrap());
        symlink_dir(orig, link).expect("Couldn't create symlink");
    }
    for &file in files {
        symlink_file(FILES[0], server.path().join(file)).expect("Couldn't create symlink");
    }
    for &file in broken {
        symlink_file("souldnt-exist.xxx", server.path().join(file))
            .expect("Couldn't create symlink");
    }

    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;

    for &entry in files.into_iter().chain(dirs) {
        let node = parsed
            .find(|x: &Node| x.name().unwrap_or_default() == "a" && x.text() == entry)
            .next();
        assert_eq!(node.is_none(), no_symlinks);
        if no_symlinks {
            continue;
        }

        let node = node.unwrap();
        assert_eq!(node.attr("href").unwrap().strip_prefix("/").unwrap(), entry);
        reqwest::blocking::get(server.url().join(&entry)?)?.error_for_status()?;
        if entry.ends_with("/") {
            assert_eq!(node.attr("class").unwrap(), "directory");
        } else {
            assert_eq!(node.attr("class").unwrap(), "file");
        }
    }
    for &entry in broken {
        assert!(parsed.find(|x: &Node| x.text() == entry).next().is_none());
    }

    Ok(())
}

#[rstest]
fn serves_requests_with_randomly_assigned_port(tmpdir: TempDir) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg(tmpdir.path())
        .arg("-p")
        .arg("0".to_string())
        .stdout(Stdio::piped())
        .spawn()?;

    sleep(Duration::from_secs(1));
    child.kill()?;

    let output = child.wait_with_output().expect("Failed to read stdout");
    let all_text = String::from_utf8(output.stdout)?;

    let re = Regex::new(r"http://127.0.0.1:(\d+)").unwrap();
    let caps = re.captures(all_text.as_str()).unwrap();
    let port_num = caps.get(1).unwrap().as_str().parse::<u16>().unwrap();

    assert!(port_num > 0);

    Ok(())
}

#[rstest]
fn serves_requests_custom_index_notice(tmpdir: TempDir, port: u16) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg("--index=not.html")
        .arg("-p")
        .arg(port.to_string())
        .arg(tmpdir.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    sleep(Duration::from_secs(1));

    child.kill()?;
    let output = child.wait_with_output().expect("Failed to read stdout");
    let all_text = String::from_utf8(output.stderr);

    assert!(all_text
        .unwrap()
        .contains("The file 'not.html' provided for option --index could not be found."));

    Ok(())
}
