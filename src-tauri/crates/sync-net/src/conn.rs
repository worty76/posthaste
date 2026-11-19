//! Length-prefixed JSON frame I/O over an iroh bidirectional QUIC stream.

use iroh::endpoint::{Connection, RecvStream, SendStream};
use posthaste_core::sync::{check_frame_len, encode_frame, Message};

use crate::NetError;

/// One authenticated peer-to-peer sync stream.
pub struct Stream {
    remote_id: String,
    conn: Connection,
    send: SendStream,
    recv: RecvStream,
}

impl Stream {
    pub(crate) fn new(
        remote_id: String,
        conn: Connection,
        send: SendStream,
        recv: RecvStream,
    ) -> Self {
        Self {
            remote_id,
            conn,
            send,
            recv,
        }
    }

    /// The authenticated iroh `EndpointId` of the remote device.
    pub fn remote_id(&self) -> &str {
        &self.remote_id
    }

    /// Writes one length-prefixed JSON message to the stream.
    pub async fn send(&mut self, msg: &Message) -> Result<(), NetError> {
        let frame = encode_frame(msg).map_err(|e| NetError::Protocol(e.to_string()))?;
        self.send
            .write_all(&frame)
            .await
            .map_err(|e| NetError::Io(e.to_string()))?;
        Ok(())
    }

    /// Reads the next length-prefixed JSON message, returning `Ok(None)` on
    /// clean EOF. Refuses frames over `MAX_FRAME_BYTES` before allocating.
    pub async fn recv(&mut self) -> Result<Option<Message>, NetError> {
        let mut len_buf = [0u8; 4];
        match self.recv.read_exact(&mut len_buf).await {
            Ok(()) => {}
            Err(iroh::endpoint::ReadExactError::FinishedEarly(0)) => return Ok(None),
            Err(e) => return Err(NetError::Io(e.to_string())),
        }
        let len = u32::from_be_bytes(len_buf) as usize;
        check_frame_len(len).map_err(|e| NetError::Protocol(e.to_string()))?;
        let mut body = vec![0u8; len];
        self.recv
            .read_exact(&mut body)
            .await
            .map_err(|e| NetError::Io(e.to_string()))?;
        let msg: Message =
            serde_json::from_slice(&body).map_err(|e| NetError::Protocol(e.to_string()))?;
        Ok(Some(msg))
    }

    /// Finishes the send half and waits briefly for the peer to observe our
    /// final frames before closing the underlying QUIC connection.
    pub async fn finish_and_close(mut self) {
        let _ = self.send.finish();
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            self.send.stopped(),
        )
        .await;
        self.conn.close(0u32.into(), b"done");
    }

    /// Finishes the send half so the peer sees a clean stream end.
    pub fn finish(&mut self) {
        let _ = self.send.finish();
    }

    /// Closes the underlying QUIC connection cleanly.
    pub fn close(self) {
        self.conn.close(0u32.into(), b"done");
    }
}
