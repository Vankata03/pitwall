//! Best-effort mDNS. LAN URLs and QR remain authoritative.
use crate::ingest::config::Config;
pub struct Discovery {
    daemon: Option<mdns_sd::ServiceDaemon>,
    name: Option<String>,
}
impl Default for Discovery {
    fn default() -> Self {
        Self::new()
    }
}
impl Discovery {
    pub fn new() -> Self {
        Self {
            daemon: mdns_sd::ServiceDaemon::new().ok(),
            name: None,
        }
    }
    pub fn advertise(&mut self, config: &Config) {
        let Some(daemon) = &self.daemon else { return };
        if let Some(name) = self.name.take() {
            let _ = daemon.unregister(&name);
        }
        // A restricted listener must not advertise inaccessible addresses.
        let addresses = if config.http.bind.is_unspecified() {
            String::new()
        } else {
            config.http.bind.to_string()
        };
        if config.http.bind.is_loopback() {
            return;
        }
        if let Ok(info) = mdns_sd::ServiceInfo::new(
            "_http._tcp.local.",
            "pitwall",
            "pitwall.local.",
            addresses.as_str(),
            config.http.port,
            &[("path", "/")][..],
        ) {
            let name = info.get_fullname().to_string();
            let info = if config.http.bind.is_unspecified() {
                info.enable_addr_auto()
            } else {
                info
            };
            if daemon.register(info).is_ok() {
                self.name = Some(name);
            }
        }
    }
}
impl Drop for Discovery {
    fn drop(&mut self) {
        if let Some(d) = &self.daemon {
            if let Some(n) = &self.name {
                let _ = d.unregister(n);
            }
            let _ = d.shutdown();
        }
    }
}
