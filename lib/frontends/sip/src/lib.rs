//! SIP UAC frontend — agent reaches kamailio over cluster-internal
//! Consul DNS on plain TCP. Standard SIP digest auth: the Go
//! provisioner inserted a `subscriber` row at agent-create time
//! (username = the agent's PGP fingerprint, password generated and
//! stashed in vault under `sip-frontend/sip-password`); the agent
//! reads that password at boot and answers kamailio's 401 challenge.
//! No TLS, no mTLS, no PGP signing at this hop — kamailio is reachable
//! only from sibling pots over the cluster LAN.

pub mod frontend;
mod transport;

pub use frontend::SipFrontend;
