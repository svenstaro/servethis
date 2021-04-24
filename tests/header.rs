mod fixtures;

use fixtures::{server, Error};
use rstest::rstest;

#[rstest(headers,
    case(vec!["x-info: 123".to_string()]),
    case(vec!["x-info1: 123".to_string(), "x-info2: 345".to_string()])
)]
fn custom_header_set(headers: Vec<String>) -> Result<(), Error> {
    let server = server(headers.iter().flat_map(|h| vec!["--header", h]));
    let resp = reqwest::blocking::get(server.url())?;

    for header in headers {
        let mut header_split = header.splitn(2, ':');
        let header_name = header_split.next().unwrap();
        let header_value = header_split.next().unwrap().trim();
        assert_eq!(resp.headers().get(header_name).unwrap(), header_value);
    }

    Ok(())
}
