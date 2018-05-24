use error::JfsXmlError;
use failure::Error;
use file::File;
use folder::Folder;
use fromxml::FromXml;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::str::FromStr;

#[derive(Debug)]
pub enum Object {
    File(File),
    Folder(Folder),
}

impl FromStr for Object {
    type Err = Error;

    #[inline]
    fn from_str(s: &str) -> Result<Object, Error> {
        let mut buf = Vec::new();
        let mut reader = Reader::from_str(s);
        reader.trim_text(true).expand_empty_elements(true);

        loop {
            match reader.read_event(&mut buf)? {
                Event::Start(element) => if element.name() == b"folder" {
                    debug!("Found folder tag, parsing folder");
                    return Ok(Object::Folder(Folder::from_xml(
                        &mut reader,
                        element.attributes(),
                    )?));
                } else if element.name() == b"file" {
                    debug!("Found folder tag, parsing file");
                    return Ok(Object::File(File::from_xml(
                        &mut reader,
                        element.attributes(),
                    )?));
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
