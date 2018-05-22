//! A sample Hyper client using this crate for TLS connections
//!
//! You can test this out by running:
//!
//!     cargo run --example hyper-client
//!
//! and on stdout you should see rust-lang.org's headers and web page.
//!
//! Note that there's also the `hyper-tls` crate which may be useful.

extern crate failure;
extern crate futures;
extern crate hyper;
extern crate hyper_tls;
extern crate keyring;
extern crate mime;
extern crate pretty_env_logger;
extern crate quick_xml;
extern crate rpassword;
extern crate tokio_core;
#[macro_use]
extern crate log;
extern crate jfs;

use failure::Error;
use futures::{Future, Stream};

fn parse_xml(xml: &str) -> Result<bool, Error> {
    use jfs::File;

    println!("{:?}", xml.parse::<File>());

    //let mut txt : Vec<u8> = Vec::new();
    //let mut buf = Vec::new();
    //let mut count = 0;
    //loop {
    //some_tag(&mut reader, &mut buf)?;
    //}
    //println!("Found {} start events", count);
    //println!("Text events: {:?}", txt);
    Ok(true)
}

fn get_object(uri: hyper::Uri, username: String, password: String) -> hyper::Request {
    use hyper::header::{Authorization, Basic};
    use hyper::{Method, Request};

    let mut request = Request::new(Method::Get, uri);

    request.headers_mut().set(Authorization(Basic {
        username,
        password: Some(password),
    }));
    request
}

fn main() {
    use hyper::header::ContentType;
    use hyper::mime;

    pretty_env_logger::init();

    let mut core = tokio_core::reactor::Core::new().unwrap();
    let handle = core.handle();
    let client = hyper::Client::configure()
        .connector(hyper_tls::HttpsConnector::new(4, &handle).unwrap())
        .build(&handle);

    let content_type = vec![ContentType(mime::TEXT_XML)];

    let username = env!("JOTTA_USER");
    let keyring = keyring::Keyring::new("jotta-rest", &username);

    let password = keyring.get_password().unwrap_or_else(|_| {
        let pw = rpassword::prompt_password_stderr(&format!("Password for user {}: ", username))
            .unwrap();
        keyring
            .set_password(&pw)
            .expect("Couldn't set password to keyring :/");
        pw
    });

    // TODO: so refactoren, dass man das beliebig kombinieren kann und ich einen generator hab
    let work = client
        .request(get_object(
            "https://www.jottacloud.com/jfs/jotta@mescharet.de/Jotta/Sync/test123/data/113da8b2b96edce1fce7429b6214a042cf0457383c1d96fe7da82e4ca94977a5" //?mode=bin"
                .parse()
                .unwrap(),
            username.to_owned(),
            password.to_owned()
        ))
        .map_err(|e| Error::from(e)) // convert to failure error :)
        .and_then(|res| {
            use futures::future::{ok, err};
            use failure::err_msg;

            debug!("Status: {}", res.status());
            println!("Headers:\n{}", res.headers());

            // matches i.e. application/xml and text/xml
            if content_type.iter().any( |t|
                    t.subtype()
                    ==  res
                          .headers().get::<ContentType>()
                          .expect("Need ContentType header")
                          .subtype()
                )
            {
                ok(res)
            } else
            {
                err(err_msg(format!("Expected ContentType {:?} not found!", content_type)))
            }
        }).and_then( |res | {
            use std::str;
            res.body().concat2().map_err(|e| Error::from(e)).and_then(move |chunk|
                parse_xml(str::from_utf8(&chunk)?)
            )
        });

    /*.and_then( |_|
        client.get("https://hyper.rs/guides/".parse().unwrap()).and_then(|res| {
        debug!("Status: {}", res.status());
        debug!("Headers:\n{}", res.headers());
        res.body().for_each(|chunk| {
            ::std::io::stdout().write_all(&chunk)
                .map(|_| ())
                .map_err(From::from)
        })
    })
    );*/
    core.run(work).unwrap();
}
