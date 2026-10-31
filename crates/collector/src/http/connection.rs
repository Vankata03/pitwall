use std::{
    future::Future,
    io,
    pin::Pin,
    task::{Context, Poll},
};

use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpStream,
    sync::watch,
};

/// Keep listener cancellation attached to the socket after an HTTP upgrade has
/// transferred it out of the HTTP connection task.
pub(super) struct Connection {
    stream: Option<TcpStream>,
    closed: Pin<Box<dyn Future<Output = ()> + Send>>,
}

impl Connection {
    pub(super) fn new(stream: TcpStream, mut shutdown: watch::Receiver<()>) -> Self {
        Self {
            stream: Some(stream),
            closed: Box::pin(async move {
                let _ = shutdown.changed().await;
            }),
        }
    }

    fn stream(self: Pin<&mut Self>, cx: &mut Context<'_>) -> io::Result<Pin<&mut TcpStream>> {
        let this = self.get_mut();
        if this.stream.is_none() || this.closed.as_mut().poll(cx).is_ready() {
            this.stream.take();
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "HTTP listener stopped",
            ));
        }
        Ok(Pin::new(this.stream.as_mut().unwrap()))
    }
}

impl AsyncRead for Connection {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.stream(cx)?.poll_read(cx, buf)
    }
}

impl AsyncWrite for Connection {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.stream(cx)?.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.stream(cx)?.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.stream(cx)?.poll_shutdown(cx)
    }
}
