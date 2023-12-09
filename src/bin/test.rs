use std::borrow::Cow;

use kvs::protocol::Value;

fn main() {
    let array = (0..10u64).map(Value::Positive).collect();
    let array = Value::Array(vec![
        Value::Bytes(Cow::Borrowed(&[0xFF][..])),
        Value::String(Cow::Borrowed("hello world")),
        Value::Negative(-0b10111),
        Value::Positive(500),
        Value::Array(array),
    ]);
    dbg!(array);
}
