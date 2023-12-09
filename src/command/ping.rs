use crate::command::Command;
use crate::error::{ProtocolError, Result};
use std::borrow::Cow;
use std::sync::Arc;

use bytes::BytesMut;
use sharded::Map;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{codec::Connection, protocol::Value};

#[derive(Debug, PartialEq, Clone)]
pub struct Ping;

impl Command for Ping {
    type ExecutionResult = Result<()>;
    async fn execute<W, R>(
        &self,
        connection: &mut Connection<R, W>,
        _: Arc<Map<BytesMut, Value<'static>>>,
    ) -> Self::ExecutionResult
    where
        W: AsyncWrite + Unpin,
        R: AsyncRead + Unpin,
    {
        let _ = connection
            .write_frame(Value::String(Cow::Borrowed("PONG")))
            .await;

        Ok(connection.flush_writer().await?)
    }

    fn encode(&self) -> Value<'_> {
        Value::Array(vec![Value::String(Cow::Borrowed("PING"))])
    }

    fn decode<'c, V: AsRef<[Value<'c>]>>(req: V) -> Result<Self>
    where
        Self: Sized,
    {
        if req.as_ref().is_empty() {
            Ok(Self)
        } else {
            Err(ProtocolError::Command)
        }
    }
}
