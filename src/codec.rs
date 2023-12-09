use crate::error::{self, ProtocolError};
use bytes::BytesMut;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufWriter},
    net::{
        tcp::{ReadHalf, WriteHalf},
        TcpStream,
    },
};

use crate::protocol::{parse, Value};

/// Wrappers around [`tokio::io::AsyncRead`] and [`tokio::io::AsyncWrite`] to work with
/// [`crate::protocol::Value`]. It uses buffered write.
///
/// After you write some value to the stream, you need to flush it manyally.
pub struct Connection<R, W> {
    pub read_half: R,
    pub write_half: BufWriter<W>,
    buf: BytesMut,
}

impl<'s> Connection<ReadHalf<'s>, WriteHalf<'s>> {
    /// Creates new connection from [`tokio::net::TcpStream`].
    pub fn from_stream(stream: &'s mut TcpStream) -> Connection<ReadHalf<'s>, WriteHalf<'s>> {
        let (read_half, write_half) = stream.split();
        Self::new(read_half, write_half)
    }
}

impl<R, W> Connection<R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    pub fn new(read_half: R, write_half: W) -> Self {
        Self {
            read_half,
            write_half: BufWriter::new(write_half),
            buf: BytesMut::new(),
        }
    }

    /// Reads some amount of bytes from the stream and parses it into [`crate::protocol::Value`].
    ///
    /// If the number of bytes is 0, returns [`crate::error::ProtocolError`].
    pub async fn read_frame(&mut self) -> error::Result<Value<'_>> {
        self.buf.clear();
        let read = self.read_half.read_buf(&mut self.buf).await?;
        if read == 0 {
            return Err(ProtocolError::ZeroRead);
        }
        Ok(parse(&self.buf[..read])?.1)
    }

    pub async fn write_frame(&mut self, data: Value<'_>) -> error::Result<()> {
        Ok(self.write_half.write_all(&data.encode()[..]).await?)
    }

    pub async fn flush_writer(&mut self) -> std::io::Result<()> {
        self.write_half.flush().await
    }
}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, sync::Arc};

    use bytes::BytesMut;
    use nom::AsBytes;
    use tokio::io::{AsyncRead, AsyncWrite};

    use crate::{
        codec::Connection,
        command::{
            entry::CommandEntry,
            get::{Get, EMPTY},
            ping::Ping,
            set::Set,
        },
        protocol::{parse, Value},
    };

    struct TestStream {
        commands: Vec<CommandEntry>,
    }

    impl AsyncRead for TestStream {
        fn poll_read(
            mut self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            match self.commands.pop() {
                Some(command) => {
                    buf.put_slice(command.encode().encode().as_bytes());
                    std::task::Poll::Ready(Ok(()))
                }
                None => std::task::Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "the stream is empty",
                ))),
            }
        }
    }

    struct TestWriter {
        values: Vec<Value<'static>>,
    }

    impl TestWriter {
        pub fn new() -> Self {
            Self { values: vec![] }
        }
    }

    impl AsyncWrite for TestWriter {
        fn poll_write(
            mut self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
            buf: &[u8],
        ) -> std::task::Poll<Result<usize, std::io::Error>> {
            let len = buf.len();
            match parse(buf) {
                Ok(value) => {
                    self.values.push(value.1.to_owned());
                    std::task::Poll::Ready(Ok(len))
                }
                Err(_) => std::task::Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "parse error",
                ))),
            }
        }

        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), std::io::Error>> {
            std::task::Poll::Ready(Ok(()))
        }

        fn poll_shutdown(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), std::io::Error>> {
            std::task::Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn ping() {
        let reader = TestStream {
            commands: vec![CommandEntry::Ping(Ping)],
        };

        let writer = TestWriter::new();

        let mut connection = Connection::new(reader, writer);
        let db = Arc::new(sharded::Map::new());

        let payload = connection.read_frame().await;
        assert!(payload.is_ok());
        let payload = payload.unwrap();
        let command = CommandEntry::parse(payload);
        assert!(command.is_ok());
        let command = command.unwrap();
        assert_eq!(command, CommandEntry::Ping(Ping));
        command.execute(&mut connection, db.clone()).await;
        assert_eq!(
            connection.write_half.get_ref().values,
            vec![Value::String(Cow::Borrowed("PONG"))]
        );
    }

    #[tokio::test]
    async fn get() {
        let reader = TestStream {
            commands: vec![
                CommandEntry::Get(Get {
                    key: BytesMut::from(&b"test2"[..]),
                }),
                CommandEntry::Get(Get {
                    key: BytesMut::from(&b"test"[..]),
                }),
            ],
        };

        let writer = TestWriter { values: vec![] };

        let mut connection = Connection::new(reader, writer);
        let db = Arc::new(sharded::Map::new());

        {
            let db = db.clone();
            let (key, mut shard) = db.write(BytesMut::from(&b"test2"[..]));
            shard.insert(key, Value::<'static>::Positive(42));
        }

        let payload = connection.read_frame().await;
        assert!(payload.is_ok());
        let payload = payload.unwrap();
        let command = CommandEntry::parse(payload);
        assert!(command.is_ok());
        let command = command.unwrap();
        assert_eq!(
            command,
            CommandEntry::Get(Get {
                key: BytesMut::from(&b"test"[..])
            })
        );
        command.execute(&mut connection, db.clone()).await;
        let payload = connection.read_frame().await;
        assert!(payload.is_ok());
        let payload = payload.unwrap();
        let command = CommandEntry::parse(payload);
        assert!(command.is_ok());
        let command = command.unwrap();
        assert_eq!(
            command,
            CommandEntry::Get(Get {
                key: BytesMut::from(&b"test2"[..])
            })
        );
        command.execute(&mut connection, db.clone()).await;
        assert_eq!(
            connection.write_half.get_ref().values,
            vec![Value::Error(Cow::Borrowed(EMPTY)), Value::Positive(42)]
        );
    }

    #[tokio::test]
    async fn set() {
        let reader = TestStream {
            commands: vec![
                CommandEntry::Set(Set {
                    key: BytesMut::from(&b"test"[..]),
                    value: Value::Positive(42),
                }),
                CommandEntry::Set(Set {
                    key: BytesMut::from(&b"test"[..]),
                    value: Value::Positive(43),
                }),
            ],
        };
        let writer = TestWriter::new();
        let mut connection = Connection::new(reader, writer);
        let db = Arc::new(sharded::Map::new());
        let payload = connection.read_frame().await;
        assert!(payload.is_ok());
        let payload = payload.unwrap();
        let command = CommandEntry::parse(payload);
        assert!(command.is_ok());
        let command = command.unwrap();
        assert_eq!(
            command,
            CommandEntry::Set(Set {
                key: BytesMut::from(&b"test"[..]),
                value: Value::Positive(43)
            })
        );
        command.execute(&mut connection, db.clone()).await;
        let payload = connection.read_frame().await;
        assert!(payload.is_ok());
        let payload = payload.unwrap();
        let command = CommandEntry::parse(payload);
        assert!(command.is_ok());
        let command = command.unwrap();
        assert_eq!(
            command,
            CommandEntry::Set(Set {
                key: BytesMut::from(&b"test"[..]),
                value: Value::Positive(42)
            })
        );
        command.execute(&mut connection, db.clone()).await;
        assert_eq!(
            connection.write_half.get_ref().values,
            vec![Value::Error(Cow::Borrowed(EMPTY)), Value::Positive(43)]
        );
    }
    #[tokio::test]
    async fn incr() {}
}
