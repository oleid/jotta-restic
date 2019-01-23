#[macro_use]
extern crate failure;

#[macro_use]
extern crate log;

#[macro_use]
extern crate serde_derive;

#[macro_use]
mod fromxml;

mod error;
mod file;
mod folder;
mod object;
mod util;

pub use crate::error::{JfsXmlError, JottaError};
pub use crate::file::File;
pub use crate::folder::Folder;
pub use crate::fromxml::FromXml;
pub use crate::object::Object;

use actix_web::client;
use actix_web::http::header::AUTHORIZATION;

use bytes::Bytes;

use failure::err_msg;
use futures::{
    future::{err, ok},
    Future, Stream,
};

pub struct JottaClient {
    authorization: String,
    base_url: String,
    username: String,
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
        let user_and_password = format!("{}:{}", username, password);
        let authorization = format!("Basic {}", base64::encode(&user_and_password));

        // It would seem, currently only Archive, Shared and Sync are supported as "mount points"
        let base_url = format!("https://www.jottacloud.com/jfs/{}/Jotta/Sync", username);

        JottaClient {
            authorization,
            base_url,
            username: username.to_owned(),
        }
    }

    fn handle_client_response(
        res: client::ClientResponse,
    ) -> impl Future<Item = Object, Error = failure::Error> {
        use actix_web::HttpMessage;
        use hyper::header::CONTENT_TYPE;

        debug!("handle_client_response status: {}", res.status());

        // matches i.e. application/xml and text/xml
        if res.headers().get(CONTENT_TYPE).map_or_else(
            || false, // TODO: do we need to support different cases, i.e. upper case here?
            |v| v.as_bytes() == b"text/xml" || v.as_bytes() == b"application/xml",
        ) {
            ok(res)
        } else {
            err(err_msg(format!(
                "Expected ContentType {:?} not found!",
                mime::TEXT_XML
            )))
        }
        .and_then(|res| {
            res.body()
                .into_stream()
                .concat2()
                .map_err(failure::Error::from)
        })
        .and_then(|ref body_bytes| parse_xml::<Object>(std::str::from_utf8(body_bytes)?))
        .map_err(failure::Error::from)
    }

    pub fn query_object(&self, path: &str) -> impl Future<Item = Object, Error = failure::Error> {
        let mut full_uri = self.base_url.clone();
        full_uri.push_str(path);

        debug!("query_object via {}", full_uri);
        let http_request = client::ClientRequest::get(full_uri)
            .header(AUTHORIZATION, self.authorization.as_str())
            .finish()
            .unwrap()
            .send();

        http_request
            .map_err(failure::Error::from)
            .and_then(JottaClient::handle_client_response)
    }

    pub fn list(&self, path: &str) -> impl Future<Item = Folder, Error = failure::Error> {
        self.query_object(path).and_then(|obj| match obj {
            Object::Folder(dir) => ok(dir),
            Object::File(_) => err(err_msg("Not a directory")),
        })
    }

    pub fn upload<S>(
        &self,
        path: &str,
        data: S,
    ) -> impl Future<Item = Object, Error = failure::Error>
    where
        S: Stream<Item = Bytes, Error = failure::Error> + 'static,
    {
        use futures::Stream;
        use md5;
        use mpart_async::{ByteStream, MultipartRequest};

        // TODO: find out, if there is a way to do a streaming upload;
        // this API needs the total file size and some md5 sum, which we don't
        // know in advance, of course.

        // URL is   https://up.jottacloud.com/jfs/[...]

        let mut s = "https://up.jottacloud.com/jfs/".to_owned();
        s.push_str(&self.username);
        s.push_str("/Jotta/Sync/");
        s.push_str(path);

        debug!("upload via '{}'", s);

        let auth = self.authorization.clone(); // TODO: can we get rid of cloning? Oo

        data.concat2()
            .map_err(|e| failure::Error::from(e))
            .and_then(move |bytes| {
                let digest = format!("{:x}", md5::compute(bytes.as_ref()));
                let date = format!("{}", chrono::Utc::now());

                let mut mpart = MultipartRequest::default();
                mpart.add_field("cphash", &digest);
                mpart.add_field("md5", &digest);
                mpart.add_field("created", &date);
                mpart.add_field("modified", &date);
                mpart.add_stream(
                    "file",
                    "blupp",
                    "application/octet-stream",
                    ByteStream::new(bytes.as_ref()), // a big fat note: this is required, as
                                                     // MultipartRequest only implements Stream, if it's template argument implements
                                                     // Stream to; which Bytes doesn't.
                );

                let request = client::ClientRequest::post(s)
                    .header(AUTHORIZATION, auth)
                    .header("X-Jfs-DeviceName", "Jotta")
                    .header("JSize", bytes.len().to_string())
                    .header("JMd5", digest.clone())
                    .content_type(format!(
                        "multipart/form-data; boundary={}",
                        mpart.get_boundary()
                    ))
                    .body(actix_web::Body::Streaming(Box::new(mpart.from_err())));
                debug!("Upload request: {:?}", request);

                request
                    .unwrap()
                    .send()
                    .timeout(std::time::Duration::from_secs(600)) // TODO: is there a better way?
                    .map_err(failure::Error::from)
                    .and_then(JottaClient::handle_client_response)
            })
    }

    pub fn download(&self, path: &str) -> impl Future<Item = Bytes, Error = failure::Error> {
        use actix_web::HttpMessage;
        use hyper::http::StatusCode;

        let mut full_uri = self.base_url.clone();
        full_uri.push_str(path);
        full_uri.push_str("?mode=bin");

        debug!("download via '{}'", full_uri);

        let http_request = client::ClientRequest::get(full_uri)
            .header(AUTHORIZATION, self.authorization.as_str())
            .finish()
            .unwrap()
            .send();

        http_request.map_err(failure::Error::from).and_then(|res| {
            let status_code = res.status();
            res.body()
                .into_stream()
                .concat2()
                .map_err(failure::Error::from)
                .and_then(move |body_bytes| {
                    if status_code == StatusCode::OK {
                        Ok(body_bytes)
                    } else {
                        Err(std::str::from_utf8(&body_bytes)
                            .map_err(failure::Error::from)
                            .and_then(parse_xml::<JottaError>)
                            .map(failure::Error::from)
                            .unwrap_or_else(|e| e)) // return failure in any case
                    }
                })
        })
    }

    pub fn mkdir(&self, path: &str) -> impl Future<Item = Object, Error = failure::Error> {
        // cf https://github.com/oleid/jottalib/blob/add_restic_server/src/jottalib/JFS.py

        let mut full_uri = self.base_url.clone();
        full_uri.push_str(path);
        full_uri.push_str("?mkDir=true");

        debug!("mkdir via '{}'", full_uri);

        client::ClientRequest::post(full_uri)
            .header(AUTHORIZATION, self.authorization.as_str())
            .finish()
            .unwrap()
            .send()
            .map_err(failure::Error::from)
            .and_then(JottaClient::handle_client_response)
    }

    pub fn exists(&self, path: &str) -> impl Future<Item = bool, Error = failure::Error> {
        debug!("exists '{}'", path);

        self.query_object(path)
            .and_then(|obj| match obj {
                Object::File(_) => ok(true),
                Object::Folder(_) => ok(true),
            })
            .or_else(|error: failure::Error| {
                error
                    .find_root_cause()
                    .downcast_ref::<JottaError>()
                    .and_then(|ref e| if e.code == 404 { Some(ok(false)) } else { None })
                    .unwrap_or(err(error))
            })
    }

    pub fn delete(&self, path: &str) -> impl Future<Item = Object, Error = failure::Error> {
        debug!("delete of {}", path);
        let authorization = self.authorization.clone();
        let mut full_uri = self.base_url.clone();
        full_uri.push_str(path);

        self.query_object(path)
            .and_then(move |obj| {
                full_uri.push_str(match obj {
                    Object::File(_) => "?dl=true",
                    Object::Folder(_) => "?dlDir=true",
                });
                debug!("delete via '{}'", full_uri);

                ok(full_uri)
            })
            .and_then(move |uri| {
                client::ClientRequest::post(uri)
                    .header(AUTHORIZATION, authorization)
                    .finish()
                    .unwrap()
                    .send()
                    .map_err(failure::Error::from)
                    .and_then(JottaClient::handle_client_response)
            })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use actix::prelude::*;

    struct TestFixture {
        client: JottaClient,
    }

    fn retrieve_password(username: &str) -> String {
        let keyring = keyring::Keyring::new("jotta-rest", &username);

        keyring.get_password().unwrap_or_else(|_| {
            let pw =
                rpassword::prompt_password_stderr(&format!("Password for user {}: ", username))
                    .unwrap();
            keyring
                .set_password(&pw)
                .expect("Couldn't set password to keyring :/");
            pw
        })
    }

    fn setup() -> TestFixture {
        // std::env::set_var("RUST_LOG", "jfs=debug");
        // std::env::set_var("RUST_BACKTRACE", "1");

        let _r = std::panic::catch_unwind(|| {
            pretty_env_logger::init();
        });

        let username = env!("JOTTA_USER");
        let password = retrieve_password(username);

        TestFixture {
            client: JottaClient::new(username, &password),
        }
    }

    #[macro_export]
    macro_rules! run_test {
        ($test:ident, $eq:expr) => {
            let fixture = setup();

            actix::run(|| {
                $test(&fixture).then(move |response| {
                    // <- server http response
                    info!("Response: {:?}", response);
                    assert!($eq(response));
                    System::current().stop();
                    Ok(())
                })
            });
        };
        ($test_lambda:expr,  $eq:expr) => {
            let tester = $test_lambda;
            run_test!(tester, $eq);
        };
    }

    fn obj_is_folder<E>(obj: Result<Object, E>) -> bool {
        obj.map(|v| match v {
            Object::Folder(_) => true,
            Object::File(_) => false,
        })
        .unwrap_or(false)
    }

    fn obj_is_file<E>(obj: Result<Object, E>) -> bool {
        obj.map(|v| match v {
            Object::Folder(_) => false,
            Object::File(_) => true,
        })
        .unwrap_or(false)
    }

    #[test]
    fn test_001_mkdir() {
        run_test!(
            |fixture: &TestFixture| fixture.client.mkdir("/test"),
            obj_is_folder
        );
    }

    #[test]
    fn test_011_upload() {
        run_test!(
            |fixture: &TestFixture| {
                let data = ok(Bytes::from_static("Hallo Welt".as_bytes())).into_stream();

                fixture.client.upload("/test/blupp.dat", data)
            },
            obj_is_file
        );
    }

    #[test]
    fn test_021_exists() {
        let is_true = |v: Result<bool, _>| v.unwrap_or(false);
        run_test!(
            |fixture: &TestFixture| fixture.client.exists("/test/blupp.dat"),
            is_true
        );
    }

    #[test]
    fn test_031_download() {
        run_test!(
            |fixture: &TestFixture| fixture.client.download("/test/blupp.dat"),
            |v: Result<Bytes, _>| v
                .map(|x| Bytes::from_static(b"Hallo Welt") == x)
                .unwrap_or(false)
        );
    }

    #[test]
    fn test_041_list() {
        let is_folder_test = |f: Result<Folder, _>| {
            debug!("Folder: {:?}", f);
            f.map(|folder| folder.name == "test").unwrap_or(false)
        };
        run_test!(
            |fixture: &TestFixture| fixture.client.list("/test"),
            is_folder_test
        );
    }

    #[test]
    fn test_051_delete_file() {
        run_test!(
            |fixture: &TestFixture| fixture.client.delete("/test/blupp.dat"),
            obj_is_file
        );
    }

    #[test]
    fn test_061_delete_folder() {
        run_test!(
            |fixture: &TestFixture| fixture.client.delete("/test"),
            obj_is_folder
        );
    }
}
