use std::borrow::Cow;

use bytes::BytesMut;
use kvs::protocol::Value;

fn main() {
    let map = (0..10u64)
        .map(|i| (BytesMut::from(&i.to_be_bytes()[..]), Value::Positive(i)))
        .collect();
    let array = Value::Array(vec![
        Value::Bytes(Cow::Borrowed(&[0xFF][..])),
        Value::String(Cow::Borrowed("hello world")),
        Value::Negative(-0b10111),
        Value::Positive(500),
        Value::Map(map),
    ]);
    dbg!(array);
}
