extern crate chrono;

#[macro_use]
extern crate failure;
extern crate futures;
extern crate mime;
extern crate pretty_env_logger;
extern crate quick_xml;
#[macro_use]
extern crate log;

mod auth_error;
mod error;
mod file;
mod fromxml;
mod util;

pub use file::File;
pub use fromxml::FromXml;
