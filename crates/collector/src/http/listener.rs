use std::{io, net::SocketAddr};

use axum::{Extension, Router, extract::ConnectInfo};
use hyper_util::{rt::TokioIo, service::TowerToHyperService};
use tokio::{
    net::TcpListener,
    sync::watch,
    task::{JoinHandle, JoinSet},
};

/// Own a collector HTTP listener. The caller supplies the configured bind address
/// and port (v1 defaults to `0.0.0.0:8777`). Binding errors are returned to the caller.
pub struct HttpListener {
    running: Option<Running>,
    router: Router,
}

struct Running {
    addr: SocketAddr,
    stop: watch::Sender<()>,
    task: JoinHandle<io::Result<()>>,
}

impl HttpListener {
    pub async fn bind(addr: SocketAddr, router: Router) -> io::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        Ok(Self {
            running: Some(Running::start(listener, router.clone())?),
            router,
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.running
            .as_ref()
            .map(|running| running.addr)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "HTTP listener is stopped"))
    }

    /// Rebind without changing routes. Bind the replacement first so a failed
    /// port change leaves the current listener available. On systems that reject
    /// overlapping binds on one port, release the old bind and restore it on failure.
    pub async fn restart(&mut self, addr: SocketAddr) -> io::Result<SocketAddr> {
        if self.local_addr().ok() == Some(addr) {
            return Ok(addr);
        }
        let previous = self.local_addr().ok();
        let listener = match TcpListener::bind(addr).await {
            Ok(listener) => listener,
            Err(error)
                if error.kind() == io::ErrorKind::AddrInUse
                    && previous.is_some_and(|old| old.port() == addr.port()) =>
            {
                self.stop().await?;
                match TcpListener::bind(addr).await {
                    Ok(listener) => listener,
                    Err(error) => {
                        let old = previous.unwrap();
                        let restored = TcpListener::bind(old).await.map_err(|restore_error| {
                            io::Error::new(restore_error.kind(), format!("HTTP rebind to {addr} failed: {error}; restoring {old} also failed: {restore_error}"))
                        })?;
                        self.running = Some(Running::start(restored, self.router.clone())?);
                        return Err(error);
                    }
                }
            }
            Err(error) => return Err(error),
        };
        self.stop().await?;
        self.running = Some(Running::start(listener, self.router.clone())?);
        self.local_addr()
    }

    /// Disconnect screens, stop accepting requests, and release the listener.
    pub async fn shutdown(mut self) -> io::Result<()> {
        self.stop().await
    }

    async fn stop(&mut self) -> io::Result<()> {
        if let Some(running) = self.running.take() {
            drop(running.stop);
            running.task.await.map_err(io::Error::other)??;
        }
        Ok(())
    }
}

impl Running {
    fn start(listener: TcpListener, router: Router) -> io::Result<Self> {
        let addr = listener.local_addr()?;
        let (stop, mut shutdown) = watch::channel(());
        let task = tokio::spawn(async move {
            let mut connections = JoinSet::new();
            loop {
                tokio::select! {
                    _ = shutdown.changed() => break,
                    accepted = listener.accept() => {
                        let (stream, peer) = accepted?;
                        let service = TowerToHyperService::new(router.clone().layer(Extension(ConnectInfo(peer))));
                        let stream = super::connection::Connection::new(stream, shutdown.clone());
                        let mut closed = shutdown.clone();
                        connections.spawn(async move {
                            tokio::select! {
                                _ = hyper::server::conn::http1::Builder::new()
                                    .serve_connection(TokioIo::new(stream), service).with_upgrades() => {},
                                _ = closed.changed() => {},
                            }
                        });
                    }
                    _ = connections.join_next(), if !connections.is_empty() => {},
                }
            }
            // Joining aborted tasks ensures old handlers and sockets are gone before restart returns.
            connections.abort_all();
            while connections.join_next().await.is_some() {}
            Ok(())
        });
        Ok(Self { addr, stop, task })
    }
}
