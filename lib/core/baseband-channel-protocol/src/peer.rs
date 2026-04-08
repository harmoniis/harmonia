use crate::sexp::{sexp_optional_string, sexp_string};

#[derive(Debug, Clone)]
pub struct PeerRef {
    pub id: String,
    pub origin_fp: Option<String>,
    pub agent_fp: Option<String>,
    pub device_id: Option<String>,
    pub platform: Option<String>,
    pub device_model: Option<String>,
    pub app_version: Option<String>,
    pub a2ui_version: Option<String>,
}

impl PeerRef {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            origin_fp: None,
            agent_fp: None,
            device_id: None,
            platform: None,
            device_model: None,
            app_version: None,
            a2ui_version: None,
        }
    }

    pub fn to_sexp(&self) -> String {
        format!(
            "(:id {}{}{}{}{}{}{})",
            sexp_string(&self.id),
            sexp_optional_string("origin-fp", self.origin_fp.as_deref()),
            sexp_optional_string("agent-fp", self.agent_fp.as_deref()),
            sexp_optional_string("device-id", self.device_id.as_deref()),
            sexp_optional_string("platform", self.platform.as_deref()),
            sexp_optional_string("device-model", self.device_model.as_deref()),
            sexp_optional_string("app-version", self.app_version.as_deref())
                + &sexp_optional_string("a2ui-version", self.a2ui_version.as_deref())
        )
    }
}

#[derive(Debug, Clone)]
pub struct ConversationRef {
    pub id: String,
}

impl ConversationRef {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }

    pub fn to_sexp(&self) -> String {
        format!("(:id {})", sexp_string(&self.id))
    }
}

#[derive(Debug, Clone)]
pub struct OriginContext {
    pub node_id: String,
    pub node_label: Option<String>,
    pub node_role: Option<String>,
    pub channel_class: Option<String>,
    pub node_key_id: Option<String>,
    pub transport_security: Option<String>,
    pub remote: bool,
}

impl OriginContext {
    pub fn to_sexp(&self) -> String {
        format!(
            "(:node-id {}{}{}{}{}{} :remote {})",
            sexp_string(&self.node_id),
            sexp_optional_string("node-label", self.node_label.as_deref()),
            sexp_optional_string("node-role", self.node_role.as_deref()),
            sexp_optional_string("channel-class", self.channel_class.as_deref()),
            sexp_optional_string("node-key-id", self.node_key_id.as_deref()),
            sexp_optional_string("transport-security", self.transport_security.as_deref()),
            crate::sexp::sexp_bool(self.remote)
        )
    }
}

#[derive(Debug, Clone)]
pub struct SessionContext {
    pub id: String,
    pub label: Option<String>,
}

impl SessionContext {
    pub fn to_sexp(&self) -> String {
        format!(
            "(:id {}{})",
            sexp_string(&self.id),
            sexp_optional_string("label", self.label.as_deref())
        )
    }
}
