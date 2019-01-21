// This file is part of rss.
//
// Copyright © 2015-2017 The rust-syndication Developers
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the MIT License and/or Apache 2.0 License.
use std::io::BufRead;

use quick_xml::events::attributes::Attributes;
use quick_xml::Reader;

use failure::Error;

pub trait FromXml: Sized {
    const TAG: &'static str;

    fn from_xml<R: BufRead>(reader: &mut Reader<R>, atts: Attributes) -> Result<Self, Error>;
}

#[macro_export]
macro_rules! impl_from_str {
    ($T:ty) => {
        impl FromStr for $T {
            type Err = failure::Error;

            #[inline]
            fn from_str(s: &str) -> Result<Self, failure::Error> {
                use std::str::from_utf8;

                let mut buf = Vec::new();
                let mut reader = Reader::from_str(s);
                reader.trim_text(true).expand_empty_elements(true);

                loop {
                    match reader.read_event(&mut buf)? {
                        Event::Start(element) => {
                            if element.name() == Self::TAG.as_bytes() {
                                debug!("Found folder tag with attrib, parsing {}", Self::TAG);
                                debug!(
                                    "attributes auth_errorvalues: {:?}",
                                    element
                                        .attributes()
                                        .map(|a| from_utf8(&a.unwrap().value).unwrap().to_owned())
                                        .collect::<Vec<_>>()
                                );
                                return Self::from_xml(&mut reader, element.attributes());
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
    };
}
