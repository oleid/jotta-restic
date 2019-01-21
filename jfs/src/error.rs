use crate::fromxml::FromXml;
use crate::util::*;
use quick_xml::events::{attributes::Attributes, Event};
use quick_xml::Reader;
use std::io::BufRead;
use std::str::FromStr;

#[derive(Debug, Fail)]
pub enum JfsXmlError {
    #[fail(display = "Early end of file")]
    UnexpectedEndOfFile,
    #[fail(display = "Unexpected tag while parsing XML: {}", tag)]
    UnexpectedTag { tag: String },
}

#[derive(Default, Debug, Fail)]
#[fail(
    display = "Jottacloud sent error code {}: {}, {}.",
    code, message, reason
)]
pub struct JottaError {
    pub message: String,
    pub reason: String,
    pub code: usize,
}

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "Error while communicating mit JottaCloud: {}.", _0)]
    Remote(JottaError),
    #[fail(display = "Error while parsing Jottas answer: {}", _0)]
    Parser(JfsXmlError),
}

impl From<JottaError> for Error {
    fn from(error: JottaError) -> Self {
        Error::Remote(error)
    }
}

impl From<JfsXmlError> for Error {
    fn from(error: JfsXmlError) -> Self {
        Error::Parser(error)
    }
}

impl FromXml for JottaError {
    const TAG: &'static str = "error";

    fn from_xml<R: BufRead>(
        reader: &mut Reader<R>,
        _attrs: Attributes,
    ) -> Result<Self, failure::Error> {
        use failure::err_msg;
        use std::str::from_utf8;

        let mut v = JottaError::default();
        let mut buf = Vec::new();

        loop {
            match reader.read_event(&mut buf)? {
                Event::Start(element) => {
                    debug!("New element: {}", from_utf8(element.name())?);
                    match element.name() {
                        b"message" => {
                            v.message = element_text(reader)?
                                .ok_or(err_msg("Couldn't pase field message of error"))?
                        }
                        b"reason" => {
                            v.reason = element_text(reader)?
                                .ok_or(err_msg("Couldn't pase field reason of error"))?
                        }
                        b"currentRevision" => (), // ignore begin of revision tag
                        b"code" => {
                            v.code = element_usize(reader)?
                                .ok_or(err_msg("Couldn't parse field code of error"))?
                        }
                        b"cause" => (),
                        b"hostname" => (),
                        b"x-id" => (),
                        n => {
                            return Err(JfsXmlError::UnexpectedTag {
                                tag: from_utf8(n)?.to_owned(),
                            }
                            .into());
                        }
                    }
                }
                Event::End(element) => {
                    if element.name() != b"error" {
                        debug!("Closing element {}, continue", from_utf8(element.name())?);
                        continue;
                    } else {
                        debug!("Closing element error, we're done here.");
                        break;
                    }
                }
                Event::Eof => return Err(JfsXmlError::UnexpectedEndOfFile.into()),
                _ => {}
            }

            buf.clear();
        }

        Ok(v)
    }
}

impl_from_str!(JottaError);

#[test]
fn test_401() {
    let _error = r#"
    <error>
        <code>401</code>
        <message>org.springframework.security.authentication.BadCredentialsException: Bad credentials</message>
        <reason>Unauthorized</reason>
        <cause></cause>
        <hostname>dn-125</hostname>
        <x-id>096492164813</x-id>
    </error>"#.parse::<JottaError>().unwrap();
}

#[test]
fn test_404() {
    let _e404 = r#"
    <error>
    <code>404</code>
    <message>no.jotta.backup.errors.NoSuchMountPointException</message>
    <reason>Not Found</reason>
    <cause></cause>
    <hostname>Backup2-backup2-get-oldgluster-dp1-7</hostname>
    <x-id>001886477694</x-id>
    </error>
    "#
    .parse::<JottaError>()
    .unwrap();
}
