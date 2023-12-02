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
}

#[derive(Debug, Error)]
#[error("")]
pub struct ParseError;

impl nom::error::ParseError<&[u8]> for ParseError {
    fn from_error_kind(input: &[u8], kind: nom::error::ErrorKind) -> Self {
        todo!()
    }

    fn append(input: &[u8], kind: nom::error::ErrorKind, other: Self) -> Self {
        todo!()
    }
}

pub type IResult<I, O> = std::result::Result<(I, O), nom::Err<ParseError>>;
