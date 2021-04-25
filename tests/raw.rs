mod fixtures;
mod utils;

use assert_cmd::prelude::*;
use assert_fs::fixture::TempDir;
use fixtures::{port, tmpdir, Error};
use pretty_assertions::assert_eq;
use rstest::rstest;
use select::document::Document;
use select::predicate::Class;
use select::predicate::Name;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

#[rstest]
/// The ui displays the correct wget command to download the folder recursively
fn ui_displays_wget_element(tmpdir: TempDir, port: u16) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg("-p")
        .arg(port.to_string())
        .arg(tmpdir.path())
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));
    // Ensure the links to the archives are not present
    let body = reqwest::blocking::get(format!("http://localhost:{}", port).as_str())?
        .error_for_status()?;
    let parsed = Document::from_read(body)?;
    let wget_url = parsed
        .find(Class("downloadWget"))
        .next()
        .unwrap()
        .find(Class("cmd"))
        .next()
        .unwrap()
        .text();
    assert_eq!(
        wget_url,
        format!(
            "wget -r -c -nH -np --cut-dirs=0 -R \"index.html*\" \"http://localhost:{}/?raw=true\"",
            port
        )
    );

    let body =
        reqwest::blocking::get(format!("http://localhost:{}/very/deeply/nested/", port).as_str())?
            .error_for_status()?;
    let parsed = Document::from_read(body)?;
    let wget_url = parsed
        .find(Class("downloadWget"))
        .next()
        .unwrap()
        .find(Class("cmd"))
        .next()
        .unwrap()
        .text();
    assert_eq!(wget_url, format!("wget -r -c -nH -np --cut-dirs=2 -R \"index.html*\" \"http://localhost:{}/very/deeply/nested/?raw=true\"", port));

    child.kill()?;
    Ok(())
}

#[rstest]
/// All hrefs in raw mode are links to directories or files & directories end with ?raw=true
fn raw_mode_links_to_directories_end_with_raw_true(
    tmpdir: TempDir,
    port: u16,
) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg("-p")
        .arg(port.to_string())
        .arg(tmpdir.path())
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    fn verify_a_tags(parsed: Document) {
        // Ensure all links end with ?raw=true or are files
        for node in parsed.find(Name("a")) {
            let class = node.attr("class").unwrap();

            if class == "root" || class == "directory" {
                assert!(node.attr("href").unwrap().ends_with("?raw=true"));
            } else if class == "file" {
                assert!(true);
            } else {
                println!(
                    "This node is a link and neither of class directory, root or file: {:?}",
                    node
                );
                assert!(false);
            }
        }
    }

    let urls = [
        format!("http://localhost:{}/?raw=true", port),
        format!("http://localhost:{}/very?raw=true", port),
        format!("http://localhost:{}/very/deeply/?raw=true", port),
        format!("http://localhost:{}/very/deeply/nested?raw=true", port),
    ];
    // Ensure the links to the archives are not present
    for url in urls.iter() {
        let body = reqwest::blocking::get(url)?.error_for_status()?;
        let parsed = Document::from_read(body)?;
        verify_a_tags(parsed);
    }

    child.kill()?;
    Ok(())
}
