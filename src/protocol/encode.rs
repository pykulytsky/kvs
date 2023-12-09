use super::{Major, Value, INDEFINITE_LENGTH};
use std::{borrow::Cow, collections::HashMap};

use bytes::{BufMut, BytesMut};

pub fn encode_map(map: HashMap<BytesMut, Value<'_>>, buf: &mut BytesMut) {
    let major = (Major::Map as u8) << 5;
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

pub fn encode_error(error: Cow<'_, str>, buf: &mut BytesMut) {
    let bytes = error.as_bytes();
    if let Some(first) = bytes.first().copied() {
        if bytes.len() == 1 && first < 24 {
            write_single_byte(first, buf, Major::Error as u8);
            return;
        }
    }
    let major = (Major::Error as u8) << 5;
    let major = major | bytes.len() as u8;
    buf.put_u8(major);
    buf.extend_from_slice(bytes);
}

pub fn encode_negative(n: i64, buf: &mut BytesMut) {
    if n.abs() < 24 {
        dbg!(-n);
        let major = (Major::Negative as u8) << 5;
        let major = major | -n as u8;
        buf.put_u8(major);
        return;
    }

    let mut len = (64 - (-n).leading_zeros() as usize) / 8;
    if len == 0 || (-n).leading_zeros() % 8 != 0 {
        len += 1;
    }

    let major = (Major::Negative as u8) << 5;
    let major = major | (len + 23) as u8;
    buf.put_u8(major);
    buf.put_int(-(n + 1), len);
}

pub fn encode_positive(n: u64, buf: &mut BytesMut) {
    if n < 24 {
        let major = (Major::Positive as u8) << 5;
        let major = major | n as u8;
        buf.put_u8(major);
        return;
    }
    let mut len = (64 - n.leading_zeros() as usize) / 8;
    if len == 0 || n.leading_zeros() % 8 != 0 {
        len += 1;
    }

    let major = (Major::Positive as u8) << 5;
    let major = major | (len + 23) as u8;
    buf.put_u8(major);
    buf.put_int(n as i64, len);
}

fn write_single_byte(byte: u8, buf: &mut BytesMut, major: u8) {
    let major = major << 5;
    let major = major | byte;
    buf.put_u8(major);
}

pub fn encode_bytes(bytes: Cow<'_, [u8]>, buf: &mut BytesMut) {
    let major = (Major::Bytes as u8) << 5;
    let major = major | bytes.len() as u8;
    buf.put_u8(major);
    buf.extend_from_slice(&bytes[..]);
}

pub fn encode_string(string: Cow<'_, str>, buf: &mut BytesMut) {
    let bytes = string.as_bytes();
    let major = (Major::String as u8) << 5;
    let major = major | bytes.len() as u8;
    buf.put_u8(major);
    buf.extend_from_slice(bytes);
}

pub fn encode_array(array: Vec<Value<'_>>, buf: &mut BytesMut) {
    let major = (Major::Array as u8) << 5;
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

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use crate::protocol::{ARRAY_MAJOR, INDEFINITE_LENGTH};

    use super::Value;
    use test_case::test_case;

    #[test_case(0, b"\x00")]
    #[test_case(1, b"\x01")]
    #[test_case(22, b"\x16")]
    #[test_case(23, b"\x17")]
    fn small_positive(number: u64, expected: &[u8; 1]) {
        let number = Value::Positive(number);
        let encoded_number = number.encode();
        assert_eq!(&encoded_number[..], expected);
    }

    #[test]
    fn positive() {
        let number = Value::Positive(500);
        let encoded_number = number.encode();
        assert_eq!(&encoded_number[..], b"\x19\x01\xf4");
    }

    #[test_case(0, b"\x20")]
    #[test_case(-1, b"\x21")]
    #[test_case(-2, b"\x22")]
    #[test_case(-22, b"\x36")]
    #[test_case(-23, b"\x37")]
    fn small_negative(number: i64, expected: &[u8; 1]) {
        let number = Value::Negative(number);
        let encoded_number = number.encode();
        assert_eq!(&encoded_number[..], expected);
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
