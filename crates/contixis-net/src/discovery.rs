use anyhow::Result;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use tokio::sync::mpsc;

const SERVICE_TYPE: &str = "_contixis._tcp.local.";

#[derive(Debug, Clone)]
pub struct DiscoveredHost {
    pub instance_name: String,
    pub host_id: String,
    pub addr: std::net::SocketAddr,
}

/// Rich event type that includes both discovery and removal.
pub enum DiscoveryEvent {
    Found(DiscoveredHost),
    /// The instance name (e.g. "contixis-30820190") of the host that went away.
    Lost { instance: String },
}

pub struct MdnsDiscovery {
    daemon: ServiceDaemon,
}

impl MdnsDiscovery {
    pub fn new() -> Result<Self> {
        Ok(Self { daemon: ServiceDaemon::new()? })
    }

    /// Advertise this host on the local network.
    pub fn advertise(&self, host_id: &str, port: u16) -> Result<()> {
        let instance = format!("contixis-{}", &host_id[..8.min(host_id.len())]);
        let host_fqdn = format!("{}.local.", local_hostname());
        let my_ip = IpAddr::V4(local_ipv4().unwrap_or(Ipv4Addr::UNSPECIFIED));

        let mut props: HashMap<String, String> = HashMap::new();
        props.insert("host_id".to_string(), host_id.to_string());

        let svc = ServiceInfo::new(
            SERVICE_TYPE,
            &instance,
            &host_fqdn,
            my_ip,
            port,
            props,
        )?;
        self.daemon.register(svc)?;
        tracing::info!(%instance, port, %my_ip, "mDNS service registered");
        Ok(())
    }

    /// Browse for hosts and stream discovered entries on the returned channel.
    pub fn browse(&self) -> Result<mpsc::UnboundedReceiver<DiscoveredHost>> {
        let (tx, rx) = mpsc::unbounded_channel();
        let receiver = self.daemon.browse(SERVICE_TYPE)?;

        tokio::spawn(async move {
            while let Ok(event) = receiver.recv_async().await {
                tracing::debug!(?event, "mDNS event");
                if let ServiceEvent::ServiceResolved(info) = event {
                    if let Some(host) = resolved_to_host(&info) {
                        let _ = tx.send(host);
                    }
                }
            }
        });

        Ok(rx)
    }

    /// Browse for hosts and stream both discovery and removal events.
    pub fn browse_events(&self) -> Result<mpsc::UnboundedReceiver<DiscoveryEvent>> {
        let (tx, rx) = mpsc::unbounded_channel();
        let receiver = self.daemon.browse(SERVICE_TYPE)?;

        tokio::spawn(async move {
            while let Ok(event) = receiver.recv_async().await {
                tracing::debug!(?event, "mDNS event");
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        for host in resolved_to_hosts(&info) {
                            tracing::info!(host_id = %host.host_id, addr = %host.addr, "mDNS host resolved");
                            let _ = tx.send(DiscoveryEvent::Found(host));
                        }
                    }
                    ServiceEvent::ServiceRemoved(_svc_type, fullname) => {
                        // fullname is "instance._contixis._tcp.local."
                        // Extract the instance part (everything before the first '.')
                        let instance = fullname
                            .split('.')
                            .next()
                            .unwrap_or(&fullname)
                            .to_string();
                        tracing::info!(%instance, "mDNS host removed");
                        let _ = tx.send(DiscoveryEvent::Lost { instance });
                    }
                    _ => {}
                }
            }
        });

        Ok(rx)
    }

    pub fn stop_advertise(&self, host_id: &str) -> Result<()> {
        let instance = format!(
            "contixis-{}.{}",
            &host_id[..8.min(host_id.len())],
            SERVICE_TYPE
        );
        self.daemon.unregister(&instance)?;
        Ok(())
    }
}

fn resolved_to_host(info: &ServiceInfo) -> Option<DiscoveredHost> {
    resolved_to_hosts(info).into_iter().next()
}

fn resolved_to_hosts(info: &ServiceInfo) -> Vec<DiscoveredHost> {
    let host_id = info
        .get_properties()
        .get_property_val_str("host_id")
        .unwrap_or("unknown")
        .to_string();
    let instance = info.get_fullname().to_string();

    info.get_addresses()
        .iter()
        .map(|addr| {
            let socket_addr = std::net::SocketAddr::new(*addr, info.get_port());
            DiscoveredHost {
                instance_name: instance.clone(),
                host_id: host_id.clone(),
                addr: socket_addr,
            }
        })
        .collect()
}

fn local_hostname() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "localhost".into())
}

/// Determine the primary outbound IPv4 address by binding a UDP socket.
/// No packets are actually sent.
fn local_ipv4() -> Option<Ipv4Addr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) => Some(ip),
        _ => None,
    }
}
