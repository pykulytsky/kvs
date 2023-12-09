use std::borrow::Cow;

use bytes::BytesMut;
use nom::AsBytes;

use crate::{command::Command, error::ProtocolError, protocol::Value};

pub struct Set {
    key: BytesMut,
    value: Value<'static>,
}

impl Command for Set {
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
        let prev = shard.insert(key, self.value.clone());
        match prev {
            Some(value) => {
                connection.write_frame(value).await;
            }
            None => {
                connection
                    .write_frame(Value::Error(Cow::Borrowed("NO VALUE")))
                    .await;
            }
        };
        Ok(())
    }

    fn decode<'c, V>(req: V) -> crate::error::Result<Self>
    where
        Self: Sized,
        V: AsRef<[Value<'c>]>,
    {
        match req.as_ref() {
            [Value::String(Cow::Borrowed("SET")), Value::Bytes(key), value, ..] => Ok(Self {
                key: BytesMut::from(key.as_bytes()),
                value: value.clone().to_owned(),
            }),
            _ => Err(ProtocolError::Command),
        }
    }

    fn encode(&self) -> Value<'_> {
        Value::Array(vec![
            Value::String(Cow::Borrowed("SET")),
            Value::Bytes(Cow::from(self.key.clone().as_bytes().to_vec())),
            self.value.clone().to_owned(),
        ])
    }
}
