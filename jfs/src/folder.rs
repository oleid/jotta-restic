use error::JfsXmlError;
use failure::Error;
use file::File;
use fromxml::FromXml;
use quick_xml::events::{attributes::Attributes, Event};
use quick_xml::Reader;
use std::io::BufRead;
use std::str::FromStr;
use util::*;

#[derive(Default, Debug)]
pub struct Folder {
    name: String,
    request_time: Option<TimeStamp>,
    deleted: Option<TimeStamp>,
    //path: String,
    abspath: Option<String>, // sometimes available, i.e. in subfolders
    files: Vec<File>,
    folders: Vec<Folder>,
}

impl FromXml for Folder {
    fn from_xml<R: BufRead>(reader: &mut Reader<R>, attrs: Attributes) -> Result<Self, Error> {
        use std::str::from_utf8;

        let mut file = Folder::default();
        let mut buf = Vec::new();

        for attr in attrs {
            let a = attr.unwrap();
            match a.key {
                b"name" => file.name = from_utf8(&a.value)?.to_owned(),
                b"deleted" => file.deleted = Some(parse_jotta_timestamp(from_utf8(&a.value)?)?),
                b"time" => file.request_time = Some(parse_jotta_timestamp(from_utf8(&a.value)?)?),
                b"host" => (), // ignored
                _ => debug!("Unhandled attribute {:?}", from_utf8(&a.value)),
            }
        }
        loop {
            match reader.read_event(&mut buf)? {
                Event::Start(element) => {
                    debug!("New element: {}", from_utf8(element.name()).unwrap());
                    match element.name() {
                        b"path" => (),
                        b"abspath" => file.abspath = element_text(reader)?,
                        b"folders" => file.folders = parse_list(reader, b"folders")?,
                        b"files" => file.files = parse_list(reader, b"files")?,
                        b"metadata" => (), // not really useful, ignore for now
                        n => {
                            return Err(JfsXmlError::UnexpectedTag {
                                tag: from_utf8(n)?.to_owned(),
                            }.into())
                        }
                    }
                }
                Event::End(element) => if element.name() != b"folder" {
                    debug!(
                        "Closing element {}, continue",
                        from_utf8(element.name()).unwrap()
                    );
                    continue;
                } else {
                    debug!("Closing element folder, we're done here.");
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

impl FromStr for Folder {
    type Err = Error;

    #[inline]
    fn from_str(s: &str) -> Result<Folder, Error> {
        use std::str::from_utf8;

        let mut buf = Vec::new();
        let mut reader = Reader::from_str(s);
        reader.trim_text(true).expand_empty_elements(true);

        loop {
            match reader.read_event(&mut buf)? {
                Event::Start(element) => if element.name() == b"folder" {
                    debug!("Found folder tag with attrib, parsing folder");
                    debug!(
                        "attributes values: {:?}",
                        element
                            .attributes()
                            .map(|a| from_utf8(&a.unwrap().value).unwrap().to_owned())
                            .collect::<Vec<_>>()
                    );
                    return Folder::from_xml(&mut reader, element.attributes());
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
    let _folder = r#"
        <folder name="data" time="2018-05-24-T19:50:45Z" host="dn-157">
            <path xml:space="preserve">/oleidinger/Jotta/Sync/test123</path>
            <abspath xml:space="preserve">/oleidinger/Jotta/Sync/test123</abspath>
            <folders>
                <folder name="config (2018-05-19 (2))" deleted="2018-05-18-T23:47:30Z">
                    <abspath xml:space="preserve">/oleidinger/Jotta/Sync/test123</abspath>
                </folder>
                <folder name="data"/>
                <folder name="index"/>
                <folder name="keys"/>
                <folder name="locks"/>
                <folder name="snapshots"/>
            </folders>
            <files>
                <file name="f87c4982fa8c1894ebf0fc8e980b901c5f3c1099bb667604910748f6da52d627" uuid="226cd129-3f6a-4670-9e37-7e72d4ecd34d">
                    <currentRevision>
                        <number>1</number>
                        <state>COMPLETED</state>
                        <created>2018-05-19-T00:55:56Z</created>
                        <modified>2018-05-19-T00:55:56Z</modified>
                        <mime>application/octet-stream</mime>
                        <size>4508471</size>
                        <md5>e1dc5bc4f2bec6bf866a0a463eb5c239</md5>
                        <updated>2018-05-19-T00:57:06Z</updated>
                    </currentRevision>
                </file>
                <file name="f91ba78157ea54ba87a99d9827ed54507af080df84a81db13c14b0571fb99ad3" uuid="9b009f64-8e6f-4bea-bd82-510edd7f645e">
                    <latestRevision>
                        <number>1</number>
                        <state>INCOMPLETE</state>
                        <created>2018-05-19-T00:50:09Z</created>
                        <modified>2018-05-19-T00:50:09Z</modified>
                        <mime>application/octet-stream</mime>
                        <md5>a3ee7c06817513862b5b3d9b758899af</md5>
                        <updated>2018-05-19-T00:50:09Z</updated>
                    </latestRevision>
                </file>
            </files>
            <metadata first="" max="" total="8" num_folders="6" num_files="2"/>
        </folder>"#.parse::<Folder>().unwrap();
}
