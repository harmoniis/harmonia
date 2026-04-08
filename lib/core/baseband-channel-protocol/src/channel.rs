use crate::sexp::sexp_string;

#[derive(Debug, Clone)]
pub struct ChannelRef {
    pub kind: String,
    pub address: String,
    pub label: String,
}

impl ChannelRef {
    pub fn new(kind: impl Into<String>, address: impl Into<String>) -> Self {
        let kind = kind.into();
        let address = address.into();
        let label = if address.is_empty() {
            kind.clone()
        } else {
            format!("{}:{}", kind, address)
        };
        Self {
            kind,
            address,
            label,
        }
    }

    pub fn to_sexp(&self) -> String {
        format!(
            "(:kind {} :address {} :label {})",
            sexp_string(&self.kind),
            sexp_string(&self.address),
            sexp_string(&self.label)
        )
    }
}
