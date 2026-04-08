use crate::sexp::{sexp_optional_string, sexp_string};

#[derive(Debug, Clone)]
pub struct TransportContext {
    pub kind: String,
    pub raw_address: String,
    pub raw_metadata: Option<String>,
}

impl TransportContext {
    pub fn to_sexp(&self) -> String {
        format!(
            "(:kind {} :raw-address {}{})",
            sexp_string(&self.kind),
            sexp_string(&self.raw_address),
            sexp_optional_string("raw-metadata", self.raw_metadata.as_deref())
        )
    }
}
