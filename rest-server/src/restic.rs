#[derive(Debug, Deserialize)]
pub struct CreateQuery {
    create: bool,
}

use std::sync::Arc;

use futures::future::{ok, result, Future};
use futures::Stream;

use actix_web::http::Method;

use actix_web::error::Error;
use actix_web::{AsyncResponder, FutureResponse, HttpMessage, HttpRequest, HttpResponse, Query};

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
        .and_then(|obj| {
            ok(obj
                .deleted()
                .map(|when| {
                    debug!("Only exists in trash; deleted at {}", when);
                    HttpResponse::NotFound().finish()
                })
                .unwrap_or_else(|| {
                    debug!("This object exists: {:?}", obj);
                    match obj {
                        Object::File(f) => {
                            HttpResponse::Ok().content_length(f.size as u64).finish()
                        }
                        Object::Folder(_) => HttpResponse::Ok().finish(),
                    }
                }))
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

            join_all(
                ["data", "index", "keys", "locks", "snapshots"]
                    .iter()
                    .map(move |subdir| {
                        let mut full_path = basedir.clone();
                        full_path.push('/');
                        full_path.push_str(subdir);

                        backend.mkdir(&full_path)
                    }),
            )
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
    let path = req.path();

    info!("Download request {}", path);

    //TODO: partial read

    req.state().backend.download(path).then(|res| {
        debug!("Response for download request: {:?}", res);
        match res {
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
        }
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
        if dir.deleted.is_none() {
            dir.files
                .into_iter()
                .filter_map(|file| match file.deleted {
                    Some(_) => None,
                    None => Some(DirListEntry::from(file)),
                })
                .collect()
        } else {
            Vec::new()
        }
    };

    req.state()
        .backend
        .query_object(path)
        .and_then(move |obj| {
            debug!("The following was returned:\n{:?}", obj);

            match obj {
                Object::Folder(dir) => ok(HttpResponse::Ok()
                    .content_type("application/vnd.x.restic.rest.v2")
                    .json(build_answer(dir))),
                Object::File(_) => ok(HttpResponse::MethodNotAllowed()
                    .reason("Not a directory")
                    .finish()),
            }
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
