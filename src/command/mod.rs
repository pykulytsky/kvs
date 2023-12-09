pub mod entry;
pub mod get;
pub mod ping;
pub mod set;

use std::sync::Arc;

use bytes::BytesMut;
use sharded::Map;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{codec::Connection, error::Result, protocol::Value};

pub trait Command {
    type ExecutionResult;
    fn execute<W, R>(
        &self,
        connection: &mut Connection<R, W>,
        db: Arc<Map<BytesMut, Value<'static>>>,
    ) -> impl std::future::Future<Output = Self::ExecutionResult>
    where
        R: AsyncRead + Unpin,
        W: Unpin + AsyncWrite;
    fn decode<'c, V>(req: V) -> Result<Self>
    where
        Self: Sized,
        V: AsRef<[Value<'c>]>;
    fn encode(&self) -> Value<'_>;
}
