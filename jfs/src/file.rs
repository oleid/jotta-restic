use crate::error::JfsXmlError;
use crate::fromxml::*;
use crate::util::*;
use failure::Error;
use mime::Mime;
use quick_xml::events::{attributes::Attributes, Event};
use quick_xml::Reader;
use std::io::BufRead;
use std::str::FromStr;

// TODO: Get rid of the options... currently, they are only there to get Default
#[derive(Default, Debug)]
pub struct File {
    pub name: String,
    pub uuid: String,
    pub request_time: Option<TimeStamp>,
    //path: Option<String>, // not present in folder
    pub abspath: Option<String>, // not present in folder
    pub revision: usize,
    pub state: Option<TransferState>,
    pub created: Option<TimeStamp>,
    pub modified: Option<TimeStamp>,
    pub mime: Option<Mime>,
    pub size: usize,
    pub md5: String,
    pub updated: Option<TimeStamp>,
}

impl FromXml for File {
    const TAG: &'static str = "file";

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
                        b"revisions" => {
                            break;
                        } // TODO: this will fail, if they ever change order.
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
                            }
                            .into());
                        }
                    }
                }
                Event::End(element) => {
                    if element.name() != b"file" {
                        debug!("Closing element {}, continue", from_utf8(element.name())?);
                        continue;
                    } else {
                        debug!("Closing element file, we're done here.");
                        break;
                    }
                }
                Event::Eof => return Err(JfsXmlError::UnexpectedEndOfFile.into()),
                _ => {}
            }

            buf.clear();
        }

        Ok(file)
    }
}

impl_from_str!(File);

#[test]
fn test_from_str() {
    let _file = r#"
<file name="blupp.dat" uuid="1502fdd0-c24e-4acc-984a-7d1e05059ccd" time="2019-01-20-T10:03:54Z" host="Backup2-backup2-get-oldgluster-dp1-2">
  <path xml:space="preserve">/oleidinger/Jotta/Sync/test</path>
  <abspath xml:space="preserve">/oleidinger/Jotta/Sync/test</abspath>
  <currentRevision>
    <number>3</number>
    <state>COMPLETED</state>
    <created>2019-01-20-T10:01:03Z</created>
    <modified>2019-01-20-T10:01:03Z</modified>
    <mime>application/octet-stream</mime>
    <size>10</size>
    <md5>5c372a32c9ae748a4c040ebadc51a829</md5>
    <updated>2019-01-20-T10:01:03Z</updated>
  </currentRevision>
  <revisions>
    <revision>
      <number>2</number>
      <state>COMPLETED</state>
      <created>2019-01-20-T07:47:19Z</created>
      <modified>2019-01-20-T07:47:19Z</modified>
      <mime>application/octet-stream</mime>
      <size>10</size>
      <md5>5c372a32c9ae748a4c040ebadc51a829</md5>
      <updated>2019-01-20-T07:47:19Z</updated>
    </revision>
    <revision>
      <number>1</number>
      <state>COMPLETED</state>
      <created>2019-01-20-T07:19:52Z</created>
      <modified>2019-01-20-T07:19:52Z</modified>
      <mime>application/octet-stream</mime>
      <size>10</size>
      <md5>5c372a32c9ae748a4c040ebadc51a829</md5>
      <updated>2019-01-20-T07:19:52Z</updated>
    </revision>
  </revisions>
</file>
"#.parse::<File>().unwrap();
}
