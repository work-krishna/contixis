pub mod framing;
pub mod transport;
pub mod discovery;
pub mod host_session;
pub mod agent_session;

pub use framing::{FrameReader, FrameWriter, Frame};
pub use transport::{
    make_host_tls_config, make_host_tls_config_open,
    make_agent_tls_config, make_agent_tls_config_insecure,
    make_agent_transport,
};
pub use discovery::MdnsDiscovery;
pub use host_session::{HandshakeEvent, HandshakeInfo, HostSession};
pub use agent_session::AgentSession;
