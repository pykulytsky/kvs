use crate::error::{self, ProtocolError};
use bytes::BytesMut;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufWriter};

use crate::protocol::{parse, Value};

pub struct Connection<'c, R, W>
where
    R: AsyncRead,
    W: AsyncWrite + 'c,
{
    pub read_half: &'c mut R,
    pub write_half: &'c mut BufWriter<W>,
    buf: BytesMut,
}

impl<'c, R, W> Connection<'c, R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin + 'c,
{
    pub fn new(read_half: &'c mut R, write_half: &'c mut BufWriter<W>) -> Self {
        Self {
            read_half,
            write_half,
            buf: BytesMut::new(),
        }
    }

    pub async fn read_frame(&'c mut self) -> error::Result<Value<'c>> {
        let read = self.read_half.read_buf(&mut self.buf).await?;
        if read == 0 {
            return Err(ProtocolError::ZeroRead);
        }
        Ok(parse(&self.buf[..])?.1)
    }

    pub async fn write_frame(&mut self, data: Value<'_>) -> error::Result<()> {
        Ok(self.write_half.write_all(&data.encode()[..]).await?)
    }

    pub async fn flush_writer(&mut self) -> std::io::Result<()> {
        self.write_half.flush().await
    }
}
