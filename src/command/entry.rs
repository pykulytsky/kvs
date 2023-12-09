use crate::{
    command::{
        decr::{Decr, DecrBy},
        incr::{Incr, IncrBy},
        Command,
    },
    error::ProtocolError,
};
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

#[derive(Debug, PartialEq, Clone)]
pub enum CommandEntry {
    Ping(Ping),
    Get(Get),
    Set(Set),
    Incr(Incr),
    IncrBy(IncrBy),
    Decr(Decr),
    DecrBy(DecrBy),
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
            "INCR" => Ok(Self::Incr(Incr::decode(&array[1..])?)),
            "INCRBY" => Ok(Self::IncrBy(IncrBy::decode(&array[1..])?)),
            "DECR" => Ok(Self::Decr(Decr::decode(&array[1..])?)),
            "DECRBY" => Ok(Self::DecrBy(DecrBy::decode(&array[1..])?)),
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
            CommandEntry::Incr(i) => i.execute(connection, db).await,
            CommandEntry::IncrBy(i) => i.execute(connection, db).await,
            CommandEntry::Decr(d) => d.execute(connection, db).await,
            CommandEntry::DecrBy(d) => d.execute(connection, db).await,
        };
        let _ = connection.flush_writer().await;
    }

    pub fn encode(self) -> Value<'static> {
        match self {
            CommandEntry::Ping(p) => p.encode().to_owned(),
            CommandEntry::Get(g) => g.encode().to_owned(),
            CommandEntry::Set(s) => s.encode().to_owned(),
            CommandEntry::Incr(i) => i.encode().to_owned(),
            CommandEntry::IncrBy(i) => i.encode().to_owned(),
            CommandEntry::Decr(d) => d.encode().to_owned(),
            CommandEntry::DecrBy(d) => d.encode().to_owned(),
        }
    }
}
