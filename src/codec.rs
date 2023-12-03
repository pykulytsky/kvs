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

pub struct Connection<R, W> {
    pub read_half: R,
    pub write_half: BufWriter<W>,
    buf: BytesMut,
}

impl<'s> Connection<ReadHalf<'s>, WriteHalf<'s>> {
    pub fn from_stream(stream: &'s mut TcpStream) -> Connection<ReadHalf<'s>, WriteHalf<'s>> {
        let (read_half, write_half) = stream.split();
        Self::new(read_half, BufWriter::new(write_half))
    }
}

impl<R, W> Connection<R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    pub fn new(read_half: R, write_half: BufWriter<W>) -> Self {
        Self {
            read_half,
            write_half,
            buf: BytesMut::new(),
        }
    }

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

    use super::Connection;
    use tokio::net::TcpStream;

    #[tokio::test]
    async fn it_works() {
        let mut stream = TcpStream::connect("").await.unwrap();
        let c = Connection::from_stream(&mut stream);
    }
}
