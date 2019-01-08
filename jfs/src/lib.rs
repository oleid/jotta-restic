#[macro_use]
extern crate failure;

#[macro_use]
extern crate log;

mod auth_error;
mod error;
mod file;
mod folder;
mod fromxml;
mod object;
mod util;

pub use crate::auth_error::JottaError;
pub use crate::file::File;
pub use crate::folder::Folder;
pub use crate::fromxml::FromXml;
pub use crate::object::Object;

use failure::{err_msg, Error};
use futures::{future::{err, ok},
              Future,
              Stream};

type HttpsClient =
    hyper::Client<hyper_tls::HttpsConnector<hyper::client::HttpConnector>, hyper::Body>;

//let content_type = layz_static!{ vec![ContentType(mime::TEXT_XML)]};

pub struct JottaClient {
    authorization: String,
    base_url: String, // TODO: turn to Bytes
    client: HttpsClient,
}

use std::fmt::Debug;
use std::str::FromStr;

fn parse_xml<T: FromStr + Debug>(xml: &str) -> Result<T, failure::Error>
where
    <T as std::str::FromStr>::Err: std::fmt::Debug,
{
    use failure::err_msg;

    let result = xml.parse::<T>();
    debug!("Parsing resulted in {:?}", result);

    result.map_err(|e| err_msg(format!("Error parsing : {:?}", e)))
}

impl JottaClient {
    pub fn new(username: &str, password: &str) -> JottaClient {
        let https = hyper_tls::HttpsConnector::new(4).unwrap();
        let client = hyper::Client::builder().build(https);

        let user_and_password = format!("{}:{}", username, password);
        let authorization = format!("Basic {}", base64::encode(&user_and_password));
        let base_url = format!("https://www.jottacloud.com/jfs/{}/Jotta", username);

        JottaClient {
            authorization,
            base_url,
            client,
        }
    }

    pub fn query_object(&self, path: &str) -> impl Future<Item = Object, Error = failure::Error> {
        use hyper::header::AUTHORIZATION;
        use hyper::{Body, Request, Uri};

        // TODO: there must be a better way than parsing uris all the time

        let http_request = {
            let mut full_uri = self.base_url.clone();
            full_uri.push_str(path); //?mode=bin"
            full_uri.parse().map_err(|e| Error::from(e))
        }.and_then(|uri: hyper::Uri| {
            Request::get(uri)
                    .header(AUTHORIZATION, self.authorization.as_str())
                    .body(Body::empty()) // no body needed
                    .map_err(|e| Error::from(e))
        })
            // TODO: not really sure how to get rid of this unwrap, resp. how to properly
            // propagate the error one lvl up. creating a future::result and issuing the request
            // from a future::AndThen yields to livetime issues
            .unwrap();

        self.client
            .request(http_request)
            .map_err(|e| Error::from(e))
            .and_then(|res| {
                use hyper::header::CONTENT_TYPE;
                let text_xml = mime::TEXT_XML;

                debug!("Status: {}", res.status());

                // matches i.e. application/xml and text/xml
                if res.headers().get(CONTENT_TYPE).map_or_else(
                    || false, // TODO: do we need to support different cases, i.e. upper case here?
                    |v| v.as_bytes() == b"text/xml" || v.as_bytes() == b"application/xml",
                ) {
                    ok(res)
                } else {
                    err(err_msg(format!(
                        "Expected ContentType {:?} not found!",
                        text_xml
                    )))
                }
            })
            .and_then(|res| res.into_body().concat2().map_err(|e| Error::from(e)))
            .and_then(|ref chunk| {
                use std::str;
                parse_xml::<Object>(str::from_utf8(chunk)?)
            })
    }

    pub fn list(&self, path: &str) -> impl Future<Item = Folder, Error = failure::Error> {
        self.query_object(path).and_then(|obj| match obj {
            Object::Folder(dir) => ok(dir),
            Object::File(_) => err(err_msg("Not a directory")),
        })
    }
}
