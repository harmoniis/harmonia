mod ffi;
pub mod mesh;
pub mod model;
pub mod transport;

pub use mesh::{discover_peers, register_node};
pub use model::{MeshMessage, MeshMessageType, NodeCapabilities, NodeId, NodeInfo};
pub use transport::{poll_messages, send_message};
