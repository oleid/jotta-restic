use chrono::ParseError as ChronoParseError;
use chrono::{format, format::Item, DateTime, Utc};

use failure::Error;

use mime::Mime;
use quick_xml::events::Event;
use quick_xml::Reader;

use std::io::BufRead;
use std::str::FromStr;

pub type TimeStamp = DateTime<Utc>;

#[derive(Debug)]
pub enum TransferState {
    Incomplete,
    Completed,
}

impl FromStr for TransferState {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use failure::err_msg;
        match s {
            "COMPLETED" => Ok(TransferState::Completed),
            "INCOMPLETE" => Ok(TransferState::Incomplete),
            _ => Err(err_msg(format!("Couldn't convert {} to TransferState", s))),
        }
    }
}

// For some reason I don't know, the string "%F-T%T%Z" doesnt work.
// c.f. https://docs.rs/chrono/0.4.0/chrono/format/strftime/index.html
// doesn't work. Hence, I do it like from_str() for DateTime

const JOTTA_TIMESTAMP_FMT_ITEMS: &'static [Item<'static>] = &[
    Item::Space(""),
    Item::Numeric(format::Numeric::Year, format::Pad::Zero),
    Item::Space(""),
    Item::Literal("-"),
    Item::Space(""),
    Item::Numeric(format::Numeric::Month, format::Pad::Zero),
    Item::Space(""),
    Item::Literal("-"),
    Item::Space(""),
    Item::Numeric(format::Numeric::Day, format::Pad::Zero),
    Item::Space(""),
    Item::Literal("-"),
    Item::Space(""),
    Item::Literal("T"),
    Item::Space(""),
    Item::Numeric(format::Numeric::Hour, format::Pad::Zero),
    Item::Space(""),
    Item::Literal(":"),
    Item::Space(""),
    Item::Numeric(format::Numeric::Minute, format::Pad::Zero),
    Item::Space(""),
    Item::Literal(":"),
    Item::Space(""),
    Item::Numeric(format::Numeric::Second, format::Pad::Zero),
    Item::Fixed(format::Fixed::Nanosecond),
    Item::Space(""),
    Item::Fixed(format::Fixed::TimezoneOffsetZ),
    Item::Space(""),
];

pub fn parse_jotta_timestamp(input: &str) -> Result<TimeStamp, ChronoParseError> {
    use chrono::format::{parse, Parsed};

    let mut parsed = Parsed::new();
    parse(
        &mut parsed,
        input,
        JOTTA_TIMESTAMP_FMT_ITEMS.iter().cloned(),
    )?;
    parsed.to_datetime().map(|s| s.with_timezone(&Utc))
}

// The following parser code is based on a utils.rs from rss crate,
// which has the following copyright:
//
// Copyright © 2015-2017 The rust-syndication Developers
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the MIT License and/or Apache 2.0 License.

pub fn element_generic<R: BufRead, T, F>(
    reader: &mut Reader<R>,
    trans: F,
) -> Result<Option<T>, Error>
where
    F: Fn(&str) -> Result<T, Error>,
{
    let mut content: Option<T> = None;
    let mut buf = Vec::new();
    let mut skip_buf = Vec::new();

    loop {
        match reader.read_event(&mut buf)? {
            Event::Start(element) => {
                reader.read_to_end(element.name(), &mut skip_buf)?;
            }
            Event::CData(element) => {
                content = Some(trans(&reader.decode(&*element))?);
            }
            Event::Text(element) => {
                content = Some(trans(&reader.decode(&element.unescaped()?))?);
            }
            Event::End(_) | Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(content)
}

pub fn element_text<R: BufRead>(reader: &mut Reader<R>) -> Result<Option<String>, Error> {
    element_generic(reader, |v| Ok(v.to_owned()))
}

pub fn element_usize<R: BufRead>(reader: &mut Reader<R>) -> Result<Option<usize>, Error> {
    element_generic(reader, |v| Ok(v.parse::<usize>()?))
}

pub fn element_timestamp<R: BufRead>(reader: &mut Reader<R>) -> Result<Option<TimeStamp>, Error> {
    element_generic(reader, |v| Ok(parse_jotta_timestamp(v)?))
}

pub fn element_mime<R: BufRead>(reader: &mut Reader<R>) -> Result<Option<Mime>, Error> {
    element_generic(reader, |v| Ok(v.parse::<Mime>()?))
}

pub fn element_transfer_state<R: BufRead>(
    reader: &mut Reader<R>,
) -> Result<Option<TransferState>, Error> {
    element_generic(reader, |v| Ok(v.parse::<TransferState>()?))
}

#[test]
pub fn test_timestamp_format() {
    use chrono::prelude::*;

    let input = "2018-05-19-T00:18:37Z";

    assert_eq!(
        parse_jotta_timestamp(input),
        Ok(Utc.ymd(2018, 05, 19).and_hms(0, 18, 37))
    )
}
