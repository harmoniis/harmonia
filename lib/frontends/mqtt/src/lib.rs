//! MQTT 5 mTLS frontend for the Harmonia agent.
//!
//! Replaces the previous `harmonia-mqtt-client` C-ABI crate. The agent
//! connects to the cluster's VerneMQ broker over native MQTT 5 + mTLS;
//! topic structure follows the Phase 5b convention:
//!
//! ```text
//! harmonia/{agent_fp}/inbox/{client_fp}        (client → agent)
//! harmonia/{agent_fp}/outbox/{client_fp}       (agent  → client)
//! harmonia/{agent_fp}/heartbeat                (agent  → cluster)
//! harmonia/{agent_fp}/system                   (provisioner ↔ agent)
//! ```
//!
//! Phase 5b1 (this crate): MQTT 5 actor frontend, mTLS, basic
//! subscribe / publish through the [`Frontend`] trait. The PGP-signing layer
//! (Phase 5b3) plugs in atop this crate when the trust-store actor lands;
//! payloads are unsigned JSON for now.

pub mod frontend;
mod tls;

pub use frontend::MqttFrontend;
