mod fixtures;

use fixtures::{server, Error, TestServer};
use reqwest::blocking::{multipart, Client};
use rstest::rstest;
use select::document::Document;
use select::predicate::{Attr, Text};

#[rstest(server(&["-u"]))]
fn uploading_files_works(server: TestServer) -> Result<(), Error> {
    let test_file_name = "uploaded test file.txt";

    // Before uploading, check whether the uploaded file does not yet exist.
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Text).all(|x| x.text() != test_file_name));

    // Perform the actual upload.
    let upload_action = parsed
        .find(Attr("id", "file_submit"))
        .next()
        .expect("Couldn't find element with id=file_submit")
        .attr("action")
        .expect("Upload form doesn't have action attribute");
    let form = multipart::Form::new();
    let part = multipart::Part::text("this should be uploaded")
        .file_name(test_file_name)
        .mime_str("text/plain")?;
    let form = form.part("file_to_upload", part);

    let client = Client::new();
    client
        .post(server.url().join(upload_action)?)
        .multipart(form)
        .send()?
        .error_for_status()?;

    // After uploading, check whether the uploaded file is now getting listed.
    let body = reqwest::blocking::get(server.url())?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Text).any(|x| x.text() == test_file_name));

    Ok(())
}

#[rstest]
fn uploading_files_is_prevented(server: TestServer) -> Result<(), Error> {
    let test_file_name = "uploaded test file.txt";

    // Before uploading, check whether the uploaded file does not yet exist.
    let body = reqwest::blocking::get(server.url())?.error_for_status()?;
    let parsed = Document::from_read(body)?;
    assert!(parsed.find(Text).all(|x| x.text() != test_file_name));

    // Ensure the file upload form is not present
    assert!(parsed.find(Attr("id", "file_submit")).next().is_none());

    // Then try to upload anyway
    let form = multipart::Form::new();
    let part = multipart::Part::text("this should not be uploaded")
        .file_name(test_file_name)
        .mime_str("text/plain")?;
    let form = form.part("file_to_upload", part);

    let client = Client::new();
    // Ensure uploading fails and returns an error
    assert!(client
        .post(server.url().join("/upload?path=/")?)
        .multipart(form)
        .send()?
        .error_for_status()
        .is_err());

    // After uploading, check whether the uploaded file is now getting listed.
    let body = reqwest::blocking::get(server.url())?;
    let parsed = Document::from_read(body)?;
    assert!(!parsed.find(Text).any(|x| x.text() == test_file_name));

    Ok(())
}
