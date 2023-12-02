use crate::protocol::{
    Value, ARRAY_MAJOR, BYTES_MAJOR, ERROR_MAJOR, INDEFINITE_LENGTH, MAP_MAJOR, NEGATIVE_MAJOR,
    POSITIVE_MAJOR, STRING_MAJOR,
};
use std::{borrow::Cow, collections::HashMap};

use crate::error::IResult;
use bytes::BytesMut;
use nom::{
    bytes::complete::{tag, take},
    combinator::{map, map_res},
    multi::{count, many_till},
    number::complete::be_u8,
    sequence::tuple,
};

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
}
