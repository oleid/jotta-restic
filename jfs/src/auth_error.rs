use crate::error::JfsXmlError;
use failure::Error;
use crate::fromxml::FromXml;
use quick_xml::events::{attributes::Attributes, Event};
use quick_xml::Reader;
use std::io::BufRead;
use std::str::FromStr;
use crate::util::*;

#[derive(Default, Debug)]
pub struct JottaError {
    message: String,
    reason: String,
    code: usize,
}

impl FromXml for JottaError {
    fn from_xml<R: BufRead>(reader: &mut Reader<R>, _attrs: Attributes) -> Result<Self, Error> {
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
                            }.into())
                        }
                    }
                }
                Event::End(element) => if element.name() != b"error" {
                    debug!("Closing element {}, continue", from_utf8(element.name())?);
                    continue;
                } else {
                    debug!("Closing element error, we're done here.");
                    break;
                },
                Event::Eof => return Err(JfsXmlError::UnexpectedEndOfFile.into()),
                _ => {}
            }

            buf.clear();
        }

        Ok(v)
    }
}

impl FromStr for JottaError {
    type Err = Error;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Error> {
        use std::str::from_utf8;

        let mut buf = Vec::new();
        let mut reader = Reader::from_str(s);
        reader.trim_text(true).expand_empty_elements(true);

        loop {
            match reader.read_event(&mut buf)? {
                Event::Start(element) => if element.name() == b"error" {
                    debug!("Found file tag with attrib, parsing file");
                    debug!(
                        "attributes values: {:?}",
                        element
                            .attributes()
                            .map(|a| from_utf8(&a.unwrap().value).unwrap().to_owned())
                            .collect::<Vec<_>>()
                    );
                    return JottaError::from_xml(&mut reader, element.attributes());
                } else {
                    debug!("Some other tag, will continue");
                    continue;
                },
                Event::End(_) => return Err(JfsXmlError::UnexpectedEndOfFile.into()),
                Event::Eof => return Err(JfsXmlError::UnexpectedEndOfFile.into()),
                _ => {}
            }
        }
    }
}

#[test]
fn test_from_str() {
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
