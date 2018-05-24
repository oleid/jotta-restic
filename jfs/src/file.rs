use error::JfsXmlError;
use failure::Error;
use fromxml::FromXml;
use mime::Mime;
use quick_xml::events::{attributes::Attributes, Event};
use quick_xml::Reader;
use std::io::BufRead;
use std::str::FromStr;
use util::*;

// TODO: Get rid of the options... currently, they are only there to get Default
#[derive(Default, Debug)]
pub struct File {
    name: String,
    uuid: String,
    request_time: Option<TimeStamp>,
    //path: Option<String>, // not present in folder
    abspath: Option<String>, // not present in folder
    revision: usize,
    state: Option<TransferState>,
    created: Option<TimeStamp>,
    modified: Option<TimeStamp>,
    mime: Option<Mime>,
    size: usize,
    md5: String,
    updated: Option<TimeStamp>,
}

impl FromXml for File {
    fn from_xml<R: BufRead>(reader: &mut Reader<R>, attrs: Attributes) -> Result<Self, Error> {
        use failure::err_msg;
        use std::str::from_utf8;

        let mut file = File::default();
        let mut buf = Vec::new();

        for attr in attrs {
            let a = attr?;
            match a.key {
                b"name" => file.name = from_utf8(&a.value)?.to_owned(),
                b"uuid" => file.uuid = from_utf8(&a.value)?.to_owned(),
                b"time" => file.request_time = Some(parse_jotta_timestamp(from_utf8(&a.value)?)?),
                b"host" => (), // ignored
                _ => debug!("Unhandled attribute {:?}", from_utf8(&a.value)),
            }
        }
        loop {
            match reader.read_event(&mut buf)? {
                Event::Start(element) => {
                    debug!("New element: {}", from_utf8(element.name())?);
                    match element.name() {
                        b"abspath" => file.abspath = element_text(reader)?,
                        b"path" => (),            // same as abspath, hence ignore
                        b"currentRevision" => (), // ignore begin of revision tag
                        b"latestRevision" => (),  // same for incomplete file
                        b"number" => {
                            file.revision = element_usize(reader)?
                                .ok_or(err_msg("Couldn't parse revision of file"))?
                        }
                        b"state" => file.state = element_transfer_state(reader)?,
                        b"created" => file.created = element_timestamp(reader)?,
                        b"modified" => file.modified = element_timestamp(reader)?,
                        b"size" => {
                            file.size = element_usize(reader)?
                                .ok_or(err_msg("Couldn't parse size of file"))?
                        }
                        b"mime" => file.mime = element_mime(reader)?,
                        b"md5" => {
                            file.md5 = element_text(reader)?
                                .ok_or(err_msg("Couldn't parse md5 sum of file"))?
                        }
                        b"updated" => file.updated = element_timestamp(reader)?,
                        n => {
                            return Err(JfsXmlError::UnexpectedTag {
                                tag: from_utf8(n)?.to_owned(),
                            }.into())
                        }
                    }
                }
                Event::End(element) => if element.name() != b"file" {
                    debug!("Closing element {}, continue", from_utf8(element.name())?);
                    continue;
                } else {
                    debug!("Closing element file, we're done here.");
                    break;
                },
                Event::Eof => return Err(JfsXmlError::UnexpectedEndOfFile.into()),
                _ => {}
            }

            buf.clear();
        }

        Ok(file)
    }
}

impl FromStr for File {
    type Err = Error;

    #[inline]
    fn from_str(s: &str) -> Result<File, Error> {
        use std::str::from_utf8;

        let mut buf = Vec::new();
        let mut reader = Reader::from_str(s);
        reader.trim_text(true).expand_empty_elements(true);

        loop {
            match reader.read_event(&mut buf)? {
                Event::Start(element) => if element.name() == b"file" {
                    debug!("Found file tag with attrib, parsing file");
                    debug!(
                        "attributes values: {:?}",
                        element
                            .attributes()
                            .map(|a| from_utf8(&a.unwrap().value).unwrap().to_owned())
                            .collect::<Vec<_>>()
                    );
                    return File::from_xml(&mut reader, element.attributes());
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
    let _file = r#"
    <file name="113da8b2b96edce1fce7429b6214a042cf0457383c1d96fe7da82e4ca94977a5" uuid="8eaf0a81-4877-40e0-be97-ed73ffc9621d" time="2018-05-20-T08:20:47Z" host="dn-132">
        <path xml:space="preserve">/oleidinger/Jotta/Sync/test123/data</path>
        <abspath xml:space="preserve">/oleidinger/Jotta/Sync/test123/data</abspath>
        <currentRevision>
            <number>1</number>
            <state>COMPLETED</state>
            <created>2018-05-19-T00:18:37Z</created>
            <modified>2018-05-19-T00:18:37Z</modified>
            <mime>application/octet-stream</mime>
            <size>27613</size>
            <md5>c392f4b2819e8777d310f235a4535acc</md5>
            <updated>2018-05-19-T00:18:37Z</updated>
        </currentRevision>
    </file>"#.parse::<File>().unwrap();
}
