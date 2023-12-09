use std::borrow::Cow;

use bytes::BytesMut;
use nom::AsBytes;

use crate::{command::Command, protocol::Value};

#[derive(Debug, PartialEq, Clone)]
pub struct Decr {
    pub key: BytesMut,
}

impl Command for Decr {
    type ExecutionResult = crate::error::Result<()>;

    async fn execute<W, R>(
        &self,
        connection: &mut crate::codec::Connection<R, W>,
        db: std::sync::Arc<sharded::Map<bytes::BytesMut, crate::protocol::Value<'static>>>,
    ) -> Self::ExecutionResult
    where
        R: tokio::io::AsyncRead + Unpin,
        W: Unpin + tokio::io::AsyncWrite,
    {
        let (key, mut shard) = db.write(self.key.clone());
        if let Some(value) = shard.get_mut(key.clone()) {
            match value.clone() {
                Value::Positive(p) => {
                    *value = Value::Positive(p - 1);
                    let _ = connection.write_frame(Value::Positive(p - 1)).await;
                }
                Value::Negative(n) => {
                    *value = Value::Negative(n - 1);
                    let _ = connection.write_frame(Value::Negative(n - 1)).await;
                }
                _ => {
                    let _ = connection
                        .write_frame(Value::Error(Cow::from("Not a number")))
                        .await;
                }
            };
        } else {
            shard.insert(key, Value::Positive(0));
            let _ = connection.write_frame(Value::Positive(0)).await;
        }
        Ok(())
    }

    fn decode<'c, V>(req: V) -> crate::error::Result<Self>
    where
        Self: Sized,
        V: AsRef<[crate::protocol::Value<'c>]>,
    {
        match req.as_ref()[0] {
            Value::Bytes(ref b) => Ok(Self {
                key: BytesMut::from(b.as_bytes()),
            }),
            _ => Err(crate::error::ProtocolError::Command),
        }
    }

    fn encode(&self) -> crate::protocol::Value<'_> {
        Value::Array(vec![
            Value::String(Cow::from("DECR")),
            Value::Bytes(Cow::from(self.key.as_bytes())),
        ])
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct DecrBy {
    pub key: BytesMut,
    pub by: i64,
}

impl Command for DecrBy {
    type ExecutionResult = crate::error::Result<()>;

    async fn execute<W, R>(
        &self,
        connection: &mut crate::codec::Connection<R, W>,
        db: std::sync::Arc<sharded::Map<BytesMut, Value<'static>>>,
    ) -> Self::ExecutionResult
    where
        R: tokio::io::AsyncRead + Unpin,
        W: Unpin + tokio::io::AsyncWrite,
    {
        let (key, mut shard) = db.write(self.key.clone());
        if let Some(value) = shard.get_mut(key.clone()) {
            match value.clone() {
                Value::Positive(p) => {
                    *value = Value::Positive((p as i64 - self.by) as u64);
                    let _ = connection
                        .write_frame(Value::Positive((p as i64 - self.by) as u64))
                        .await;
                }
                Value::Negative(n) => {
                    *value = Value::Negative(n - self.by);
                    let _ = connection.write_frame(Value::Negative(n - self.by)).await;
                }
                _ => {
                    let _ = connection
                        .write_frame(Value::Error(Cow::from("Not a number")))
                        .await;
                }
            };
        } else {
            shard.insert(key, Value::Positive(0));
            let _ = connection.write_frame(Value::Positive(0)).await;
        }
        Ok(())
    }

    fn decode<'c, V>(req: V) -> crate::error::Result<Self>
    where
        Self: Sized,
        V: AsRef<[Value<'c>]>,
    {
        match req.as_ref() {
            [Value::Bytes(ref b), Value::Negative(by)] => Ok(Self {
                key: BytesMut::from(b.as_bytes()),
                by: *by,
            }),
            _ => Err(crate::error::ProtocolError::Command),
        }
    }

    fn encode(&self) -> Value<'_> {
        Value::Array(vec![
            Value::String(Cow::from("DECRBY")),
            Value::Bytes(Cow::from(self.key.as_bytes())),
            Value::Negative(self.by),
        ])
    }
}
