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

extern crate keyring;
extern crate mime;
extern crate pretty_env_logger;
extern crate quick_xml;
extern crate rpassword;
extern crate tokio_core;
#[macro_use]
extern crate log;
extern crate jfs;

use futures::future::{ok, Future};

fn retrieve_password(username: &str) -> String {
    let keyring = keyring::Keyring::new("jotta-rest", &username);

    keyring.get_password().unwrap_or_else(|_| {
        let pw = rpassword::prompt_password_stderr(&format!("Password for user {}: ", username))
            .unwrap();
        keyring
            .set_password(&pw)
            .expect("Couldn't set password to keyring :/");
        pw
    })
}

fn main() {
    pretty_env_logger::init();

    let mut core = tokio_core::reactor::Core::new().unwrap();

    let username = env!("JOTTA_USER");
    let client = jfs::JottaClient::new(username, &retrieve_password(username));

    match core.run(client.list("/Sync/test123/data/")) {
        Ok(folder) => println!("We got {:?}", folder),
        Err(e) => println!("Processing error {}", e),
    }
}
