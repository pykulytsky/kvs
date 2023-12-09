pub mod encode;
pub mod parse;

pub use parse::parse;

use std::str::Utf8Error;
use std::{borrow::Cow, collections::HashMap};

use bytes::BytesMut;

pub const POSITIVE_MAJOR: u8 = 0b000;
pub const NEGATIVE_MAJOR: u8 = 0b001;
pub const BYTES_MAJOR: u8 = 0b010;
pub const STRING_MAJOR: u8 = 0b011;
pub const ARRAY_MAJOR: u8 = 0b100;
pub const ERROR_MAJOR: u8 = 0b101;
pub const MAP_MAJOR: u8 = 0b110;
pub const FLOAT_MAJOR: u8 = 0b111;

pub enum Major {
    Positive = 0b000,
    Negative = 0b001,
    Bytes = 0b010,
    String = 0b011,
    Array = 0b100,
    Error = 0b101,
    Map = 0b110,
    Float = 0b111,
}

impl TryFrom<u8> for Major {
    type Error = Utf8Error;

    fn try_from(value: u8) -> Result<Self, Utf8Error> {
        match value {
            0b000 => Ok(Major::Positive),
            0b001 => Ok(Major::Negative),
            0b010 => Ok(Major::Bytes),
            0b011 => Ok(Major::String),
            0b100 => Ok(Major::Array),
            0b101 => Ok(Major::Error),
            0b110 => Ok(Major::Map),
            0b111 => Ok(Major::Float),
            _ => todo!(),
        }
    }
}

pub const INDEFINITE_LENGTH: u8 = 31;

/// [CBOR](https://www.rfc-editor.org/rfc/rfc8949.html)-like binary format.
///
/// In general, type representation in this format consists of the first byte and (possibly) data
/// in proceeding bytes.
///
/// First byte is divided into 2 parts:
/// - major type (the high-order 3 bits)
/// - additional information (the low-order 5 bits)
///
/// By default no allocation required for parsing, to get owned value use
/// [`Value::to_owned`] or [`Value::clone`]
#[derive(Eq, PartialEq)]
pub enum Value<'input, B = u8, S = str>
where
    [B]: ToOwned<Owned = Vec<B>>,
    S: ToOwned<Owned = String> + ?Sized,
{
    Positive(u64),
    Negative(i64),
    Bytes(Cow<'input, [B]>),
    String(Cow<'input, S>),
    Array(Vec<Value<'input, B, S>>),
    Map(HashMap<BytesMut, Value<'input, B, S>>),
    Error(Cow<'input, S>),
}

impl<'input, B, S> Value<'input, B, S>
where
    B: 'input,
    [B]: ToOwned<Owned = Vec<B>>,
    S: ToOwned<Owned = String> + ?Sized + 'input,
{
    pub fn to_owned(self) -> Value<'static, B, S> {
        match self {
            Value::Positive(p) => Value::Positive(p),
            Value::Negative(n) => Value::Negative(n),
            Value::Bytes(b) => Value::Bytes(Cow::Owned(b.into_owned())),
            Value::String(s) => Value::String(Cow::Owned(s.into_owned())),
            Value::Array(array) => Value::Array(
                array
                    .into_iter()
                    .map(|i| i.to_owned())
                    .collect::<Vec<Value<'static, B, S>>>(),
            ),
            Value::Map(map) => Value::Map(
                map.into_iter()
                    .map(|(k, v)| (k.to_owned(), v.to_owned()))
                    .collect::<HashMap<BytesMut, Value<'static, B, S>>>(),
            ),
            Value::Error(e) => Value::Error(Cow::Owned(e.into_owned())),
        }
    }

    pub fn first_byte(&self) -> u8 {
        match self {
            Value::Positive(_) => todo!(),
            Value::Negative(_) => todo!(),
            Value::Bytes(_) => todo!(),
            Value::String(_) => todo!(),
            Value::Array(_) => todo!(),
            Value::Map(_) => todo!(),
            Value::Error(_) => todo!(),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Value::Positive(_) => 8,
            Value::Negative(_) => 8,
            Value::Bytes(b) => b.len(),
            Value::String(s) => s.clone().into_owned().as_bytes().len(),
            Value::Array(array) => array.iter().map(|i| i.len()).sum(),
            Value::Map(map) => map.iter().map(|(k, v)| k.len() + v.len()).sum(),
            Value::Error(e) => e.clone().into_owned().as_bytes().len(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<'input, B, S> std::fmt::Debug for Value<'input, B, S>
where
    B: std::fmt::Debug + 'input,
    [B]: ToOwned<Owned = Vec<B>>,
    S: ToOwned<Owned = String> + ?Sized + std::fmt::Debug + 'input,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Positive(n) => {
                write!(f, "p:{:?}", n)
            }
            Value::Negative(n) => {
                write!(f, "n:{:?}", n)
            }
            Value::Bytes(b) => {
                write!(f, "b:{:?}", b)
            }
            Value::String(s) => {
                write!(f, "s:{:?}", s)
            }
            Value::Array(array) => f.debug_list().entries(array.iter()).finish(),
            Value::Map(map) => f.debug_map().entries(map.iter()).finish(),
            Value::Error(error) => {
                write!(f, "e:{:?}", error)
            }
        }
    }
}

impl<'input, B> From<Vec<B>> for Value<'input, B>
where
    [B]: ToOwned<Owned = Vec<B>> + 'input,
    B: Clone,
{
    fn from(value: Vec<B>) -> Self {
        Value::Bytes(Cow::from(value))
    }
}

impl<'input, B, S> From<String> for Value<'input, B, S>
where
    S: ToOwned<Owned = String> + 'input,
    [B]: ToOwned<Owned = Vec<B>> + 'input,
{
    fn from(value: String) -> Self {
        Value::String(Cow::Owned(value))
    }
}

impl Value<'_> {
    pub fn encode(self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(self.len());

        match self {
            Value::Positive(n) => encode::encode_positive(n, &mut buf),
            Value::Negative(n) => encode::encode_negative(n, &mut buf),
            Value::Bytes(b) => encode::encode_bytes(b, &mut buf),
            Value::String(s) => encode::encode_string(s, &mut buf),
            Value::Array(array) => encode::encode_array(array, &mut buf),
            Value::Map(map) => encode::encode_map(map, &mut buf),
            Value::Error(err) => encode::encode_error(err, &mut buf),
        }

        buf
    }
}

impl<'input, B, S> Clone for Value<'input, B, S>
where
    B: 'input,
    [B]: ToOwned<Owned = Vec<B>>,
    S: ToOwned<Owned = String> + ?Sized + 'input,
{
    fn clone(&self) -> Self {
        match self {
            Self::Positive(arg0) => Self::Positive(*arg0),
            Self::Negative(arg0) => Self::Negative(*arg0),
            Self::Bytes(arg0) => Self::Bytes(arg0.clone()),
            Self::String(arg0) => Self::String(arg0.clone()),
            Self::Array(arg0) => Self::Array(arg0.clone()),
            Self::Map(arg0) => Self::Map(arg0.clone()),
            Self::Error(arg0) => Self::Error(arg0.clone()),
        }
    }
}
