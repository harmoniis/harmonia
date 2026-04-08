use crate::platform::Platform;
use crate::tools::{MineCost, Precondition, ToolKind, Domain};

pub struct Lode {
    pub id: String,
    pub kind: ToolKind,
    pub platform: Platform,
    pub domain: Domain,
    pub cost: MineCost,
    pub preconditions: Vec<Precondition>,
    pub available: bool,
    pub mine_fn: fn(&[&str]) -> Result<String, String>,
}

#[derive(Clone, Debug)]
pub struct LodeSummary {
    pub id: String,
    pub kind: ToolKind,
    pub platform: Platform,
    pub domain: Domain,
    pub cost: MineCost,
    pub available: bool,
}

fn sexp_core(id: &str, kind: ToolKind, platform: Platform, domain: Domain, available: bool) -> String {
    format!(
        ":id \"{}\" :kind {} :platform {} :domain {} :available {}",
        crate::sexp_escape(id), kind.to_sexp(), platform.to_sexp(), domain.to_sexp(),
        if available { "t" } else { "nil" },
    )
}

impl Lode {
    pub fn summary(&self) -> LodeSummary {
        LodeSummary {
            id: self.id.clone(), kind: self.kind, platform: self.platform,
            domain: self.domain, cost: self.cost, available: self.available,
        }
    }
    pub fn check_preconditions(&self) -> bool {
        self.preconditions.iter().all(|p| p.is_met())
    }
    pub fn to_sexp(&self) -> String {
        format!("({} :cost (:latency-ms {} :cpu {} :network {}))",
            sexp_core(&self.id, self.kind, self.platform, self.domain, self.available),
            self.cost.latency_ms, self.cost.cpu.to_sexp(), self.cost.network.to_sexp(),
        )
    }
}

impl LodeSummary {
    pub fn to_sexp(&self) -> String {
        format!("({})", sexp_core(&self.id, self.kind, self.platform, self.domain, self.available))
    }
}
