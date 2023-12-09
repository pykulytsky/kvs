use crate::{command::Command, error::ProtocolError};
use std::borrow::Cow;
use std::sync::Arc;

use bytes::BytesMut;
use sharded::Map;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
    codec::Connection,
    command::{get::Get, ping::Ping, set::Set},
    protocol::Value,
};

pub enum CommandEntry {
    Ping(Ping),
    Get(Get),
    Set(Set),
}

impl CommandEntry {
    pub fn parse(input: Value<'_>) -> crate::error::Result<Self> {
        let Value::Array(array) = input else {
            return Err(ProtocolError::Command);
        };
        let Value::String(Cow::Borrowed(first)) = array.first().ok_or(ProtocolError::Command)?
        else {
            return Err(ProtocolError::Command);
        };
        match *first {
            "PING" => Ok(Self::Ping(Ping::decode(&array[1..])?)),
            "GET" => Ok(Self::Get(Get::decode(&array[1..])?)),
            "SET" => Ok(Self::Set(Set::decode(&array[1..])?)),
            _ => todo!(),
        }
    }

    pub async fn execute<R, W>(
        &self,
        connection: &mut Connection<R, W>,
        db: Arc<Map<BytesMut, Value<'static>>>,
    ) where
        W: AsyncWrite + Unpin,
        R: AsyncRead + Unpin,
    {
        let _ = match self {
            CommandEntry::Ping(p) => p.execute(connection, db).await,
            CommandEntry::Get(g) => g.execute(connection, db).await,
            CommandEntry::Set(s) => s.execute(connection, db).await,
        };
    }

    pub fn encode(self) -> Value<'static> {
        match self {
            CommandEntry::Ping(p) => p.encode().to_owned(),
            CommandEntry::Get(g) => g.encode().to_owned(),
            CommandEntry::Set(s) => s.encode().to_owned(),
        }
    }
}
