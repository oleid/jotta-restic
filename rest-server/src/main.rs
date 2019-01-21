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

mod restic {

    #[derive(Debug, Deserialize)]
    pub struct CreateQuery {
        create: bool,
    }

    use std::sync::Arc;

    use futures::future::{ok, result, Future};
    use futures::Stream;

    use actix_web::http::Method;

    use actix_web::error::Error;
    use actix_web::{
        AsyncResponder, FutureResponse, HttpMessage, HttpRequest, HttpResponse, Query,
    };

    use actix_web::http::StatusCode;

    use jfs::{File as JottaFile, Folder as JottaFolder, JottaError, Object};

    #[derive(Serialize, Debug)]
    struct DirListEntry {
        name: String,
        size: usize,
    }

    impl From<JottaFile> for DirListEntry {
        fn from(f: JottaFile) -> DirListEntry {
            DirListEntry {
                name: f.name,
                size: f.size,
            }
        }
    }

    use super::AppState;

    pub fn main_handler(req: &HttpRequest<AppState>) -> FutureResponse<HttpResponse, Error> {
        // Returns “200 OK” if the repository has a configuration, an HTTP error otherwise.
        debug!("main_handler {:?}", req);

        match *req.method() {
            Method::GET => download(req).responder(),
            Method::POST => upload(req).responder(),
            Method::HEAD => exists(req).responder(),
            Method::DELETE => delete(req).responder(),
            _ => result(Ok(HttpResponse::MethodNotAllowed().finish())).responder(),
        }
    }

    // Returns “200 OK” if the blob with the given name and type is stored in the repository,
    // “404 not found” otherwise. If the blob exists,
    // the HTTP header Content-Length is set to the file size.
    pub fn exists(
        req: &HttpRequest<AppState>,
    ) -> impl Future<Item = HttpResponse, Error = actix_web::Error> + 'static {
        use futures::future::err;

        let path = req.path();
        info!("exists {:?}: ", path);

        req.state()
            .backend
            .query_object(path)
            .and_then(|obj| match obj {
                Object::File(f) => ok(HttpResponse::Ok().content_length(f.size as u64).finish()),
                Object::Folder(_) => ok(HttpResponse::Ok().finish()),
            })
            .or_else(|error: failure::Error| {
                error
                    .find_root_cause()
                    .downcast_ref::<JottaError>()
                    .and_then(|ref e| {
                        if e.code == 404 {
                            Some(ok(HttpResponse::NotFound().finish()))
                        } else {
                            None
                        }
                    })
                    .unwrap_or(err(error))
            })
            .map_err(Error::from)
    }

    /// Makes sure that the directory itself as well as certain subdirs exist
    ///
    /// These subdirs are:
    ///    ["data", "index", "keys", "locks", "snapshots"]
    pub fn create_repo(
        (opt, req): (Query<CreateQuery>, HttpRequest<AppState>),
    ) -> impl Future<Item = HttpResponse, Error = actix_web::Error> {
        let basedir = req.path().to_owned();

        info!("create repo {:?} at {:?}", opt, basedir);

        let backend: Arc<jfs::JottaClient> = req.state().backend.clone();

        req.state()
            .backend
            .mkdir(&basedir)
            .and_then(move |_| {
                use futures::future::join_all;

                join_all(["data", "index", "keys", "locks", "snapshots"].iter().map(
                    move |subdir| {
                        let mut full_path = basedir.clone();
                        full_path.push('/');
                        full_path.push_str(subdir);

                        backend.mkdir(&full_path)
                    },
                ))
            })
            .map_err(Error::from)
            .and_then(|_| Ok(HttpResponse::Ok().finish()))
    }

    // Returns the content of the blob with the given name and type if it is stored in the repository, “404 not found” otherwise.
    //
    // If the request specifies a partial read with a Range header field, then the status code of the response is 206 instead of 200 and the response only contains the specified range.
    //
    // Response format: binary/octet-stream
    pub fn download(
        req: &HttpRequest<AppState>,
    ) -> impl Future<Item = HttpResponse, Error = actix_web::Error> {
        info!("Download request {:?}", req);

        //TODO: partial read
        let path = req.path();

        req.state().backend.download(path).then(|res| match res {
            Ok(b) => Ok(HttpResponse::Ok()
                .content_type("binary/octet-stream")
                .body(b)),
            Err(error) => error
                // Forward Jotta's error codes (such as 404) to our server
                .find_root_cause()
                .downcast_ref::<JottaError>()
                .map(|ref e| {
                    Ok(HttpResponse::new(
                        StatusCode::from_u16(e.code as u16).unwrap(),
                    ))
                })
                .unwrap_or(Err(error))
                .map_err(Error::from),
        }) // Needed for streaming response
    }

    pub fn delete(
        req: &HttpRequest<AppState>,
    ) -> impl Future<Item = HttpResponse, Error = actix_web::Error> {
        // Returns “200 OK” if the repository has a configuration, an HTTP error otherwise.
        info!("delete request {:?}", req);

        let path = req.path();

        req.state()
            .backend
            .delete(path)
            .map_err(Error::from)
            .and_then(|_| Ok(HttpResponse::Ok().finish()))
    }

    pub fn upload(
        req: &HttpRequest<AppState>,
    ) -> impl Future<Item = HttpResponse, Error = actix_web::Error> {
        info!("upload request {:?}", req);

        let path = req.path();

        req.state()
            .backend
            .upload(path, req.payload().map_err(failure::Error::from))
            .map_err(Error::from)
            .and_then(|_| Ok(HttpResponse::Ok().finish()))
    }

    pub fn list_dir(
        req: &HttpRequest<AppState>,
    ) -> impl Future<Item = HttpResponse, Error = actix_web::Error> {
        //         Format:
        //        [
        //            {
        //                "name": "245bc4c430d393f74fbe7b13325e30dbde9fb0745e50caad57c446c93d20096b",
        //                "size": 2341058
        //            },
        //            [...]
        //        ]

        let path = req.path();

        info!("listing of {}", path);

        let build_answer = |dir: JottaFolder| -> Vec<DirListEntry> {
            dir.files.into_iter().map(DirListEntry::from).collect()
        };

        req.state()
            .backend
            .query_object(path)
            .and_then(move |obj| match obj {
                Object::Folder(dir) => ok(HttpResponse::Ok()
                    .content_type("application/vnd.x.restic.rest.v2")
                    .json(build_answer(dir))),
                Object::File(_) => ok(HttpResponse::MethodNotAllowed()
                    .reason("Not a directory")
                    .finish()),
            })
            .map_err(Error::from)
    }

    /*
    fn construct_response(
        resp: client::ClientResponse,
    ) -> Box<dyn Future<Item = HttpResponse, Error = Errorimpl Future<Item = HttpResponse, Error = actix_web::Error>>> {
        let mut client_resp = HttpResponse::build(resp.status());
        for (header_name, header_value) in resp.headers().iter().filter(|(h, _)| *h != "connection")
        {
            client_resp.header(header_name.clone(), header_value.clone());
        }
        if resp.chunked().unwrap_or(false) {
            Box::new(ok(client_resp.streaming(resp.payload())))
        } else {
            Box::new(
                resp.body()
                    .from_err()
                    .and_then(move |body| Ok(client_resp.body(body))),
            )
        }
    }*/
}

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
