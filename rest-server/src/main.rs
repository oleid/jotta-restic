#[macro_use]
extern crate log;

#[macro_use]
extern crate serde_derive;

use actix_web::http::Method;
use actix_web::middleware::session;
use actix_web::{middleware, pred, server, App, HttpResponse};

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

mod restic;

use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub backend: Arc<jfs::JottaClient>,
}

fn main() {
    // use std::env;
    // env::set_var("RUST_LOG", "jfs=debug");
    // env::set_var("RUST_BACKTRACE", "1");

    pretty_env_logger::init();

    let username = env!("JOTTA_USER");
    let password = retrieve_password(username);

    let sys = actix::System::new("jotta-rest-proxy");

    let _addr = server::new(move || {
        let app_state = AppState {
            backend: Arc::new(jfs::JottaClient::new(username, &password)),
        };

        App::with_state(app_state)
            .middleware(middleware::Logger::default()) // enable logger
            // cookie session middleware
            .middleware(session::SessionStorage::new(
                session::CookieSessionBackend::signed(&[0; 32]).secure(false),
            ))
            .resource("{path}/config", |r| r.route().a(restic::main_handler))
            .resource("{path}/", |r| {
                r.method(Method::POST).with_async(restic::create_repo)
            })
            .resource("{path}/{type}/", |r| {
                r.method(Method::GET).a(restic::list_dir)
            })
            .resource("{path}/{type}/{name}", |r| {
                r.route().a(restic::main_handler)
            })
            // default
            .default_resource(|r| {
                // 404 for GET request
                r.method(Method::GET).f(|_| {
                    debug!("No route found, calling GET default handler.");
                    HttpResponse::NotFound().finish()
                });

                // all requests that are not `GET`
                r.route().filter(pred::Not(pred::Get())).f(|_| {
                    debug!("No route found, calling other default handler.");
                    HttpResponse::MethodNotAllowed()
                });
            })
    })
    .bind("127.0.0.1:8080")
    .expect("Can not bind to 127.0.0.1:8080")
    .shutdown_timeout(0) // <- Set shutdown timeout to 0 seconds (default 60s)
    .start();

    println!("Starting http server: 127.0.0.1:8080");
    let _ = sys.run();
}
