use anyhow::{anyhow, Result};
use bytes::{Buf, BufMut, BytesMut};
use contixis_proto::MsgType;
use prost::Message;
use quinn::{RecvStream, SendStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Wire frame: `[u8 type][u24 BE length][N bytes payload]`
pub struct Frame {
    pub msg_type: MsgType,
    pub payload: Vec<u8>,
}

pub struct FrameWriter {
    stream: SendStream,
}

impl FrameWriter {
    pub fn new(stream: SendStream) -> Self { Self { stream } }

    pub async fn write_frame(&mut self, msg_type: MsgType, payload: &[u8]) -> Result<()> {
        let len = payload.len();
        if len > 0x00FF_FFFF {
            return Err(anyhow!("payload too large: {} bytes", len));
        }
        let mut header = [0u8; 4];
        header[0] = msg_type as u8;
        header[1] = ((len >> 16) & 0xFF) as u8;
        header[2] = ((len >>  8) & 0xFF) as u8;
        header[3] = ( len        & 0xFF) as u8;
        self.stream.write_all(&header).await?;
        if !payload.is_empty() {
            self.stream.write_all(payload).await?;
        }
        Ok(())
    }

    pub async fn write_proto<M: Message>(&mut self, msg_type: MsgType, msg: &M) -> Result<()> {
        let payload = msg.encode_to_vec();
        self.write_frame(msg_type, &payload).await
    }

    pub fn finish(mut self) -> Result<()> {
        self.stream.finish()?;
        Ok(())
    }
}

pub struct FrameReader {
    stream: RecvStream,
}

impl FrameReader {
    pub fn new(stream: RecvStream) -> Self { Self { stream } }

    pub async fn read_frame(&mut self) -> Result<Frame> {
        let mut header = [0u8; 4];
        self.stream.read_exact(&mut header).await
            .map_err(|e| anyhow!("read header: {}", e))?;

        let raw_type = header[0];
        let msg_type = MsgType::try_from(raw_type)
            .map_err(|b| anyhow!("unknown message type 0x{:02X}", b))?;

        let len = ((header[1] as usize) << 16)
                | ((header[2] as usize) <<  8)
                |  (header[3] as usize);

        let mut payload = vec![0u8; len];
        if len > 0 {
            self.stream.read_exact(&mut payload).await
                .map_err(|e| anyhow!("read payload: {}", e))?;
        }
        Ok(Frame { msg_type, payload })
    }

    pub async fn read_proto<M: Message + Default>(&mut self) -> Result<(MsgType, M)> {
        let frame = self.read_frame().await?;
        let msg = M::decode(frame.payload.as_slice())?;
        Ok((frame.msg_type, msg))
    }
}
