use thiserror::Error;

pub type Result<T> = std::result::Result<T, ProtocolError>;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("")]
    Read(#[from] tokio::io::Error),
    #[error("")]
    ZeroRead,
    #[error("")]
    Parse(#[from] nom::Err<ParseError>),
    #[error("")]
    Command,
}

#[derive(Debug, Error)]
#[error("")]
#[from(tokio::io::Error)]
pub struct ParseError;

impl nom::error::ParseError<&[u8]> for ParseError {
    fn from_error_kind(_: &[u8], _: nom::error::ErrorKind) -> Self {
        Self
    }

    fn append(_: &[u8], _: nom::error::ErrorKind, _: Self) -> Self {
        Self
    }
}

impl nom::error::FromExternalError<&[u8], std::str::Utf8Error> for ParseError {
    fn from_external_error(_: &[u8], _: nom::error::ErrorKind, _: std::str::Utf8Error) -> Self {
        Self
    }
}

pub type IResult<I, O> = std::result::Result<(I, O), nom::Err<ParseError>>;
