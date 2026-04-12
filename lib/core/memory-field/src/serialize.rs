//! Sexp serialization for FieldResult — the presentation layer.
//! Separated from computation (command.rs) and state (lib.rs).

use harmonia_actor_protocol::SexpBuilder;
use crate::command::*;

impl FieldResult {
    pub fn to_sexp(&self) -> String {
        match self {
            Self::GraphLoaded { n, edges, spectral_k, graph_version } =>
                SexpBuilder::ok()
                    .key("n").uint(*n as u64)
                    .key("edges").uint(*edges as u64)
                    .key("spectral-k").uint(*spectral_k as u64)
                    .key("graph-version").uint(*graph_version)
                    .build(),
            Self::Recalled(r) => r.to_sexp(),
            Self::Stepped(r) => r.to_sexp(),
            Self::Dreamed(r) => crate::dream::dream_report_to_sexp(r),
            Self::Bootstrapped(r) => r.to_sexp(),
            Self::Digest(d) => d.to_sexp(),
            Self::Status(r) => r.to_sexp(),
            Self::BasinStatus(r) => r.to_sexp(),
            Self::EigenmodeStatus(r) => r.to_sexp(),
            Self::EdgeCurrents(currents) => {
                let items: Vec<String> = currents.iter()
                    .map(|(a, b, c)| format!("(:a \"{}\" :b \"{}\" :current {:.4})", a, b, c))
                    .collect();
                format!("(:ok :currents ({}))", items.join(" "))
            }
            Self::DreamStats(r) => r.to_sexp(),
            Self::CurrentBasin { basin, cycle } =>
                SexpBuilder::ok()
                    .key("basin").raw(basin)
                    .key("cycle").int(*cycle)
                    .build(),
            Self::BasinRestored(r) => r.to_sexp(),
            Self::GenesisLoaded { n, edges, spectral_k, graph_version } =>
                SexpBuilder::ok()
                    .key("n").uint(*n as u64)
                    .key("edges").uint(*edges as u64)
                    .key("spectral-k").uint(*spectral_k as u64)
                    .key("graph-version").uint(*graph_version)
                    .build(),
            Self::Checkpointed { sexp } =>
                SexpBuilder::ok()
                    .key("checkpoint").raw(sexp)
                    .build(),
            Self::StateRestored => "(:ok :restored t)".to_string(),
            Self::DiskSaved { sexp } => sexp.clone(),
            Self::DiskLoaded { restored } =>
                if *restored {
                    "(:ok :disk-loaded t)".to_string()
                } else {
                    "(:ok :disk-loaded nil)".to_string()
                },
            Self::Reset => "(:ok)".to_string(),
        }
    }
}

impl StatusResult {
    pub fn to_sexp(&self) -> String {
        SexpBuilder::ok()
            .key("cycle").int(self.cycle)
            .key("graph-n").uint(self.graph_n as u64)
            .key("graph-version").uint(self.graph_version)
            .key("spectral-k").uint(self.spectral_k as u64)
            .key("basin").raw(&self.basin)
            .key("thomas-b").float(self.thomas_b, 3)
            .build()
    }
}

impl BasinStatusResult {
    pub fn to_sexp(&self) -> String {
        SexpBuilder::ok()
            .key("current").raw(&self.current)
            .key("dwell-ticks").uint(self.dwell_ticks)
            .key("coercive-energy").float(self.coercive_energy, 3)
            .key("threshold").float(self.threshold, 3)
            .build()
    }
}

impl EigenmodeResult {
    pub fn to_sexp(&self) -> String {
        let evs: Vec<String> = self.eigenvalues.iter().map(|v| format!("{v:.4}")).collect();
        SexpBuilder::ok()
            .key("eigenvalues").list(&evs)
            .key("spectral-version").uint(self.spectral_version)
            .key("graph-version").uint(self.graph_version)
            .build()
    }
}

impl DreamStatsResult {
    pub fn to_sexp(&self) -> String {
        SexpBuilder::ok()
            .key("dreams").uint(self.dreams)
            .key("pruned").uint(self.pruned)
            .key("merged").uint(self.merged)
            .key("crystallized").uint(self.crystallized)
            .key("entropy-delta").float(self.entropy_delta, 3)
            .build()
    }
}

impl SteppedResult {
    pub fn to_sexp(&self) -> String {
        format!(
            "(:ok :thomas (:x {:.3} :y {:.3} :z {:.3} :b {:.3}) :aizawa (:x {:.3} :y {:.3} :z {:.3}) :halvorsen (:x {:.3} :y {:.3} :z {:.3}) :basin {})",
            self.thomas.0, self.thomas.1, self.thomas.2, self.thomas_b,
            self.aizawa.0, self.aizawa.1, self.aizawa.2,
            self.halvorsen.0, self.halvorsen.1, self.halvorsen.2,
            self.basin,
        )
    }
}

impl BootstrapResult {
    pub fn to_sexp(&self) -> String {
        SexpBuilder::ok()
            .key("bootstrapped").raw("t")
            .key("nodes").uint(self.nodes as u64)
            .key("basin").raw(&self.basin)
            .key("dream").raw(&crate::dream::dream_report_to_sexp(&self.dream))
            .build()
    }
}

impl BasinRestoredResult {
    pub fn to_sexp(&self) -> String {
        SexpBuilder::ok()
            .key("restored").raw(&self.basin)
            .key("energy").float(self.energy, 3)
            .key("dwell").uint(self.dwell)
            .key("threshold").float(self.threshold, 3)
            .build()
    }
}
