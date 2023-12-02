use super::{
    Value, ARRAY_MAJOR, BYTES_MAJOR, ERROR_MAJOR, INDEFINITE_LENGTH, NEGATIVE_MAJOR,
    POSITIVE_MAJOR, STRING_MAJOR,
};
use std::{borrow::Cow, collections::HashMap};

use bytes::{BufMut, BytesMut};

pub fn enode_map(map: HashMap<BytesMut, Value<'_>>, buf: &mut BytesMut) {
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

pub fn encode_error(error: Cow<'_, str>, buf: &mut BytesMut) {
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

pub fn encode_negative(n: i64, buf: &mut BytesMut) {
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

pub fn encode_positive(n: u64, buf: &mut BytesMut) {
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

pub fn encode_bytes(bytes: Cow<'_, [u8]>, buf: &mut BytesMut) {
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

pub fn encode_string(string: Cow<'_, str>, buf: &mut BytesMut) {
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

pub fn encode_array(array: Vec<Value<'_>>, buf: &mut BytesMut) {
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

#[cfg(test)]
mod tests {
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