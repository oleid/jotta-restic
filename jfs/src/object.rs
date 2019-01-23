use crate::error::{JfsXmlError, JottaError};
use crate::file::File;
use crate::folder::Folder;
use crate::fromxml::FromXml;
use crate::util::*;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::str::FromStr;

#[derive(Debug, Serialize)]
pub enum Object {
    File(File),
    Folder(Folder),
}

impl Object {
    pub fn deleted(&self) -> Option<TimeStamp> {
        match self {
            Object::File(ref file) => file.deleted,
            Object::Folder(ref dir) => dir.deleted,
        }
    }
}

impl FromStr for Object {
    type Err = failure::Error;

    #[inline]
    fn from_str(s: &str) -> Result<Object, failure::Error> {
        let mut buf = Vec::new();
        let mut reader = Reader::from_str(s);
        reader.trim_text(true).expand_empty_elements(true);

        loop {
            match reader.read_event(&mut buf)? {
                Event::Start(element) => {
                    if element.name() == b"folder" {
                        debug!("Found folder tag, parsing folder");
                        return Ok(Object::Folder(Folder::from_xml(
                            &mut reader,
                            element.attributes(),
                        )?));
                    } else if element.name() == b"file" {
                        debug!("Found file tag, parsing file");
                        return Ok(Object::File(File::from_xml(
                            &mut reader,
                            element.attributes(),
                        )?));
                    } else if element.name() == b"error" {
                        debug!("Found error tag, parsing error");
                        return Err(JottaError::from_xml(&mut reader, element.attributes())?.into());
                    } else {
                        debug!("Some other tag, will continue");
                        continue;
                    }
                }
                Event::End(_) => return Err(JfsXmlError::UnexpectedEndOfFile.into()),
                Event::Eof => return Err(JfsXmlError::UnexpectedEndOfFile.into()),
                _ => {}
            }
        }
    }
}
