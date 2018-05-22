#[derive(Debug, Fail)]
pub enum JfsXmlError {
    #[fail(display = "Early end of file")]
    UnexpectedEndOfFile,
    #[fail(display = "Unexpected tag while parsing XML: {}", tag)]
    UnexpectedTag { tag: String },
}
