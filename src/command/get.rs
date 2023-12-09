use std::borrow::Cow;

use bytes::BytesMut;
use nom::AsBytes;

use crate::{command::Command, protocol::Value};

pub struct Get {
    key: BytesMut,
}

impl Command for Get {
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
        let shard = db.read(&self.key);
        match shard.1.get(shard.0) {
            Some(value) => {
                connection.write_frame(value.clone()).await;
            }
            None => {
                connection
                    .write_frame(Value::Error(Cow::Borrowed("Can not find the key")))
                    .await;
            }
        }
        Ok(())
    }

    fn decode<'c, V>(req: V) -> crate::error::Result<Self>
    where
        Self: Sized,
        V: AsRef<[crate::protocol::Value<'c>]>,
    {
        match req.as_ref() {
            [Value::String(Cow::Borrowed("GET")), Value::Bytes(b), ..] => Ok(Self {
                key: BytesMut::from(b.as_bytes()),
            }),
            _ => Err(crate::error::ProtocolError::Command),
        }
    }

    fn encode(&self) -> crate::protocol::Value<'_> {
        Value::Array(vec![
            Value::String(Cow::Borrowed("GET")),
            Value::Bytes(Cow::from(self.key.clone().as_bytes().to_vec())),
        ])
    }
}
