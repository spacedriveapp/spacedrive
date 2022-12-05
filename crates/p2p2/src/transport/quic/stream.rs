use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::{AsyncRead, AsyncWrite};
use quinn::{RecvStream, SendStream};

pub struct QuicStream {
    pub(crate) tx: SendStream,
    pub(crate) rx: RecvStream,
}

impl AsyncRead for QuicStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        <RecvStream as AsyncRead>::poll_read(Pin::new(&mut self.get_mut().rx), cx, buf)
    }
}

impl AsyncWrite for QuicStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        <SendStream as AsyncWrite>::poll_write(Pin::new(&mut self.get_mut().tx), cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        <SendStream as AsyncWrite>::poll_flush(Pin::new(&mut self.get_mut().tx), cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        <SendStream as AsyncWrite>::poll_close(Pin::new(&mut self.get_mut().tx), cx)
    }
}
