use std::{borrow::Cow, collections::HashMap};

use bytes::{BufMut, BytesMut};
use nom::{
    bytes::complete::{tag, take},
    combinator::{map, map_res},
    multi::{count, many_till},
    number::complete::be_u8,
    sequence::tuple,
    IResult,
};

pub const POSITIVE_MAJOR: u8 = 0b000;
pub const NEGATIVE_MAJOR: u8 = 0b001;
pub const BYTES_MAJOR: u8 = 0b010;
pub const STRING_MAJOR: u8 = 0b011;
pub const ARRAY_MAJOR: u8 = 0b100;
pub const ERROR_MAJOR: u8 = 0b101;
pub const MAP_MAJOR: u8 = 0b110;
pub const FLOAT_MAJOR: u8 = 0b111;

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
#[derive(Debug, Eq, PartialEq)]
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

impl Value<'_> {
    pub fn encode(self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(self.len());

        match self {
            Value::Positive(n) => encode_positive(n, &mut buf),
            Value::Negative(n) => encode_negative(n, &mut buf),
            Value::Bytes(b) => encode_bytes(b, &mut buf),
            Value::String(s) => encode_string(s, &mut buf),
            Value::Array(array) => encode_array(array, &mut buf),
            Value::Map(map) => enode_map(map, &mut buf),
            Value::Error(err) => encode_error(err, &mut buf),
        }

        buf
    }
}

#[allow(unused, dead_code)]
fn enode_map(map: HashMap<BytesMut, Value<'_>>, buf: &mut BytesMut) {
    let major = ARRAY_MAJOR << 5;
    let len = map.len();
    let major = if len < 31 {
        major | len as u8
    } else {
        INDEFINITE_LENGTH
    };
    buf.put_u8(major);
    buf.extend(map.into_iter().flat_map(|(k, v)| {
        let mut k = k;
        k.extend(v.encode());
        k
    }));
    if len >= 31 {
        buf.put_u8(0xFF);
    }
}

fn encode_error(error: Cow<'_, str>, buf: &mut BytesMut) {
    let bytes = error.as_bytes();
    if let Some(first) = bytes.first().copied() {
        if bytes.len() == 1 && first < 24 {
            let major = ERROR_MAJOR << 5;
            let major = major | first;
            buf.put_u8(major);
            return;
        }
    }
    let major = ERROR_MAJOR << 5;
    let major = major | bytes.len() as u8;
    buf.put_u8(major);
    buf.extend_from_slice(bytes);
}

fn encode_negative(n: i64, buf: &mut BytesMut) {
    if n.abs() < 24 {
        let major = NEGATIVE_MAJOR << 5;
        let major = major | -n as u8;
        buf.put_u8(major);
        return;
    }

    let mut len = (64 - (-n).leading_zeros() as usize) / 8;
    if len == 0 || (-n).leading_zeros() % 8 != 0 {
        len += 1;
    }

    let major = NEGATIVE_MAJOR << 5;
    let major = major | (len + 23) as u8;
    buf.put_u8(major);
    buf.put_int(-(n + 1), len);
}

fn encode_positive(n: u64, buf: &mut BytesMut) {
    if n < 24 {
        let major = POSITIVE_MAJOR << 5;
        let major = major | n as u8;
        buf.put_u8(major);
        return;
    }
    let mut len = (64 - n.leading_zeros() as usize) / 8;
    if len == 0 || n.leading_zeros() % 8 != 0 {
        len += 1;
    }

    let major = POSITIVE_MAJOR << 5;
    let major = major | (len + 23) as u8;
    buf.put_u8(major);
    buf.put_int(n as i64, len);
}

fn encode_bytes(bytes: Cow<'_, [u8]>, buf: &mut BytesMut) {
    if let Some(first) = bytes.first().copied() {
        if bytes.len() == 1 && first < 24 {
            let major = BYTES_MAJOR << 5;
            let major = major | first;
            buf.put_u8(major);
            return;
        }
    }
    let major = BYTES_MAJOR << 5;
    let major = major | bytes.len() as u8;
    buf.put_u8(major);
    buf.extend_from_slice(&bytes[..]);
}

fn encode_string(string: Cow<'_, str>, buf: &mut BytesMut) {
    let bytes = string.as_bytes();
    if let Some(first) = bytes.first().copied() {
        if bytes.len() == 1 && first < 24 {
            let major = STRING_MAJOR << 5;
            let major = major | first;
            buf.put_u8(major);
            return;
        }
    }
    let major = STRING_MAJOR << 5;
    let major = major | bytes.len() as u8;
    buf.put_u8(major);
    buf.extend_from_slice(bytes);
}

fn encode_array(array: Vec<Value<'_>>, buf: &mut BytesMut) {
    let major = ARRAY_MAJOR << 5;
    let len = array.len();
    let major = if len < 31 {
        major | len as u8
    } else {
        major | INDEFINITE_LENGTH
    };
    buf.put_u8(major);
    buf.extend(array.into_iter().flat_map(|i| i.encode().into_iter()));
    if len >= 31 {
        buf.put_u8(0xFF);
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

/// Parse first byte and split it into `major` and `additional` information.
pub fn parse_first_byte(input: &[u8]) -> IResult<&[u8], (u8, u8)> {
    map(be_u8, |b: u8| (b >> 5, b & 0x1F))(input)
}

pub fn parse(input: &[u8]) -> IResult<&[u8], Value<'_>> {
    let (rest, (major, size)) = parse_first_byte(input)?;
    match major {
        POSITIVE_MAJOR => parse_number(rest, size).map(|(rest, n)| (rest, Value::Positive(n))),
        NEGATIVE_MAJOR => {
            parse_number(rest, size).map(|(rest, n)| (rest, Value::Negative(-1 - n as i64)))
        }
        BYTES_MAJOR => parse_bytes(rest, size),
        STRING_MAJOR => parse_string(rest, size),
        ARRAY_MAJOR => parse_array(rest, size),
        ERROR_MAJOR => parse_error(rest, size),
        MAP_MAJOR => parse_map(rest, size),
        _ => todo!(),
    }
}

fn parse_array(input: &[u8], size: u8) -> IResult<&[u8], Value<'_>> {
    if size == INDEFINITE_LENGTH {
        return map(many_till(parse, tag(&[0xFF][..])), |items| {
            Value::Array(items.0)
        })(input);
    }
    map(
        count(parse, size as usize),
        |array: Vec<Value<'_, u8, str>>| Value::Array(array),
    )(input)
}

fn parse_map(input: &[u8], size: u8) -> IResult<&[u8], Value<'_>> {
    if size == INDEFINITE_LENGTH {
        return map(
            many_till(tuple((parse, parse)), tag(&[0xFF][..])),
            |items| {
                Value::Map(HashMap::<_, _, std::hash::RandomState>::from_iter(
                    items.0.into_iter().map(|(k, v)| (k.encode(), v)),
                ))
            },
        )(input);
    }
    map(count(tuple((parse, parse)), size as usize), |map| {
        Value::Map(HashMap::<_, _, std::hash::RandomState>::from_iter(
            map.into_iter().map(|(_, v)| (BytesMut::new(), v)),
        ))
    })(input)
}

fn parse_bytes(input: &[u8], additional: u8) -> IResult<&[u8], Value<'_>> {
    if additional < 23 {
        return Ok((input, Value::Bytes(Cow::from(vec![additional]))));
    }
    let additional = additional - 23;
    map(take(additional), |bytes: &[u8]| {
        Value::Bytes(Cow::from(bytes))
    })(input)
}

fn parse_string(input: &[u8], additional: u8) -> IResult<&[u8], Value<'_>> {
    if additional < 23 {
        return Ok((input, Value::Bytes(Cow::from(vec![additional]))));
    }
    let additional = additional - 23;
    map(
        map_res(take(additional), |bytes: &[u8]| std::str::from_utf8(bytes)),
        |s: &str| Value::String(Cow::from(s)),
    )(input)
}

fn parse_error(input: &[u8], additional: u8) -> IResult<&[u8], Value<'_>> {
    if additional < 23 {
        return Ok((input, Value::Bytes(Cow::from(vec![additional]))));
    }
    let additional = additional - 23;
    map(
        map_res(take(additional), |bytes: &[u8]| std::str::from_utf8(bytes)),
        |s: &str| Value::Error(Cow::from(s)),
    )(input)
}

/// Parses number from bytes, filling empty bytes with zeros to fit in u64.
pub fn parse_number(input: &[u8], additional: u8) -> IResult<&[u8], u64> {
    if additional < 24 {
        return Ok((input, additional as u64));
    }
    let additional = additional - 23;
    map(take(additional), |b: &[u8]| match b.len() {
        8 => {
            let mut arr = [0u8; 8];
            arr.copy_from_slice(b);
            u64::from_be_bytes(arr)
        }
        n => {
            let mut arr = [0u8; 8];
            let offset = 8 - n;
            arr[offset..].copy_from_slice(b);
            u64::from_be_bytes(arr)
        }
    })(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    mod values {
        use std::borrow::Cow;

        use crate::protocol::Value;

        use super::parse;
        #[test]
        fn small_positive() {
            let payload = [0b000_10110];
            let parsed = parse(&payload[..]);
            assert!(parsed.is_ok());
            let (rest, parsed) = parsed.unwrap();
            assert_eq!(parsed, Value::Positive(0b10110));
            assert!(rest.is_empty());
        }

        #[test]
        fn small_negative() {
            let payload = [0b001_10110];
            let parsed = parse(&payload[..]);
            assert!(parsed.is_ok());
            let (rest, parsed) = parsed.unwrap();
            assert_eq!(parsed, Value::Negative(-0b10111));
            assert!(rest.is_empty());
        }

        #[test]
        fn big_positive() {
            let payload = [0b000_11001u8, 0x01, 0xf4];
            let parsed = parse(&payload[..]);
            assert!(parsed.is_ok());
            let (rest, parsed) = parsed.unwrap();
            assert_eq!(parsed, Value::Positive(500));
            assert!(rest.is_empty());
        }

        #[test]
        fn big_negative() {
            let payload = [0b001_11001u8, 0x01, 0xf3];
            let parsed = parse(&payload[..]);
            assert!(parsed.is_ok());
            let (rest, parsed) = parsed.unwrap();
            assert_eq!(parsed, Value::Negative(-500));
            assert!(rest.is_empty());
        }

        #[test]
        fn one_byte() {
            let payload = [0b010_10110];
            let parsed = parse(&payload[..]);
            assert!(parsed.is_ok());
            let (rest, parsed) = parsed.unwrap();
            assert_eq!(parsed, Value::Bytes(Cow::Borrowed(&[22u8][..])));
            assert!(rest.is_empty());
        }

        #[test]
        fn one_big_byte() {
            let payload = [0b010_11000, 0xFF];
            let parsed = parse(&payload[..]);
            assert!(parsed.is_ok());
            let (rest, parsed) = parsed.unwrap();
            assert_eq!(parsed, Value::Bytes(Cow::Borrowed(&[0xFF][..])));
            assert!(rest.is_empty());
        }

        #[test]
        fn string() {
            let payload = [0b011_11100, 104, 101, 108, 108, 111];
            let parsed = parse(&payload[..]);
            assert!(parsed.is_ok());
            let (rest, parsed) = parsed.unwrap();
            assert_eq!(parsed, Value::String(Cow::Borrowed("hello")));
            assert!(rest.is_empty());
        }
    }

    #[test]
    fn sized_array() {
        let byte = [0b010_11000, 0xF1];
        let one_byte = [0b010_10110];
        let negative = [0b001_10110];
        let big_positive = [0b000_11001u8, 0x01, 0xf4];
        let mut payload = vec![(ARRAY_MAJOR << 5) | 0b00000100];
        payload.extend_from_slice(&byte[..]);
        payload.extend_from_slice(&one_byte[..]);
        payload.extend_from_slice(&negative[..]);
        payload.extend_from_slice(&big_positive[..]);

        let parsed = parse(&payload[..]);
        assert!(parsed.is_ok());
        let (rest, parsed) = parsed.unwrap();
        assert_eq!(
            parsed,
            Value::Array(vec![
                Value::Bytes(Cow::Borrowed(&[0xF1][..])),
                Value::Bytes(Cow::Borrowed(&[22u8][..])),
                Value::Negative(-0b10111),
                Value::Positive(500),
            ])
        );
        assert!(rest.is_empty());
    }

    #[test]
    fn unsized_array() {
        let byte = [0b010_11000, 0xFF];
        let one_byte = [0b010_10110];
        let negative = [0b001_10110];
        let big_positive = [0b000_11001u8, 0x01, 0xf4];
        let mut payload = vec![(ARRAY_MAJOR << 5) | 31];
        payload.extend_from_slice(&byte[..]);
        payload.extend_from_slice(&one_byte[..]);
        payload.extend_from_slice(&negative[..]);
        payload.extend_from_slice(&big_positive[..]);
        payload.push(0xFF);

        let parsed = parse(&payload[..]);
        assert!(parsed.is_ok());
        let (rest, parsed) = parsed.unwrap();
        assert_eq!(
            parsed,
            Value::Array(vec![
                Value::Bytes(Cow::Borrowed(&[0xFF][..])),
                Value::Bytes(Cow::Borrowed(&[22u8][..])),
                Value::Negative(-0b10111),
                Value::Positive(500),
            ])
        );
        assert!(rest.is_empty());
    }

    mod encoding {
        use std::borrow::Cow;

        use crate::protocol::{ARRAY_MAJOR, INDEFINITE_LENGTH};

        use super::Value;

        #[test]
        fn small_positive() {
            let number = Value::Positive(5);
            let encoded_number = number.encode();
            assert_eq!(&encoded_number[..], b"\x05");
        }

        #[test]
        fn positive() {
            let number = Value::Positive(500);
            let encoded_number = number.encode();
            assert_eq!(&encoded_number[..], b"\x19\x01\xf4");
        }

        #[test]
        fn small_negative() {
            let number = Value::Negative(-5);
            let encoded_number = number.encode();
            assert_eq!(&encoded_number[..], &[0b001_00101u8][..]);
        }

        #[test]
        fn negative() {
            let number = Value::Negative(-500);
            let encoded_number = number.encode();
            assert_eq!(&encoded_number[..], b"\x39\x01\xf3");
        }

        #[test]
        fn bytes() {
            let bytes = Value::<'_, u8, str>::Bytes(Cow::from(&b"hi"[..]));
            let encoded_bytes = bytes.encode();
            assert_eq!(&encoded_bytes[..], [0b010_00010, b'h', b'i']);
        }

        #[test]
        fn string() {
            let bytes = Value::<'_, u8, str>::String(Cow::from("hi"));
            let encoded_bytes = bytes.encode();
            assert_eq!(&encoded_bytes[..], [0b011_00010, b'h', b'i']);
        }

        #[test]
        fn sized_array() {
            let array = Value::Array(vec![Value::Positive(5), Value::Negative(-500)]);
            let encoded_array = array.encode();
            let mut encoded = vec![(ARRAY_MAJOR << 5) | 2];
            encoded.extend_from_slice(b"\x05");
            encoded.extend_from_slice(b"\x39\x01\xf3");
            assert_eq!(&encoded_array[..], encoded);
        }

        #[test]
        fn unsized_array() {
            let array = Value::Array(
                std::iter::repeat(Value::Positive(500))
                    .take(32)
                    .collect::<Vec<Value<'_, u8, str>>>(),
            );
            let encoded_array = array.encode();
            let mut encoded = vec![(ARRAY_MAJOR << 5) | INDEFINITE_LENGTH];
            for _ in 0..32 {
                encoded.extend_from_slice(b"\x19\x01\xf4");
            }
            encoded.extend_from_slice(b"\xFF");

            assert_eq!(&encoded_array[..], encoded);
        }
    }
}
