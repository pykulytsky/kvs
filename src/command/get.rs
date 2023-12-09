use std::borrow::Cow;

use bytes::BytesMut;
use nom::AsBytes;

use crate::{command::Command, protocol::Value};

#[derive(Debug, PartialEq, Clone)]
pub struct Get {
    pub key: BytesMut,
}

pub const EMPTY: &str = "Can not find the key";

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
                let _ = connection.write_frame(value.clone()).await;
            }
            None => {
                let _ = connection
                    .write_frame(Value::Error(Cow::Borrowed(EMPTY)))
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
        match req.as_ref()[0] {
            Value::Bytes(ref b) => Ok(Self {
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
