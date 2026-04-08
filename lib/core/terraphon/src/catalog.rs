use crate::lode::{Lode, LodeSummary};
use crate::sexp_escape;

pub struct LodeCatalog {
    lodes: Vec<Lode>,
}

impl LodeCatalog {
    pub fn new() -> Self { Self { lodes: Vec::new() } }
    pub fn register(&mut self, lode: Lode) { self.lodes.push(lode); }
    pub fn len(&self) -> usize { self.lodes.len() }
    pub fn available_count(&self) -> usize { self.lodes.iter().filter(|l| l.available).count() }
    pub fn user_tool_count(&self) -> usize {
        self.lodes.iter().filter(|l| l.kind == crate::tools::ToolKind::UserSpace).count()
    }
    pub fn find(&self, id: &str) -> Option<&Lode> {
        self.lodes.iter().find(|l| l.id == id)
    }
    pub fn find_mut(&mut self, id: &str) -> Option<&mut Lode> {
        self.lodes.iter_mut().find(|l| l.id == id)
    }
    pub fn list(&self, domain_filter: Option<crate::tools::Domain>) -> Vec<LodeSummary> {
        self.lodes.iter()
            .filter(|l| domain_filter.map_or(true, |d| l.domain == d))
            .map(|l| l.summary()).collect()
    }
    pub fn check_all_preconditions(&mut self) -> usize {
        self.lodes.iter_mut()
            .map(|lode| { lode.available = lode.check_preconditions(); lode.available })
            .filter(|&a| a).count()
    }
}

pub fn catalog_list(s: &crate::TerraphonState) -> Result<String, String> {
    let items: Vec<String> = s.catalog.list(None).iter().map(|l| l.to_sexp()).collect();
    Ok(format!("(:ok :count {} :lodes ({}))", items.len(), items.join(" ")))
}

pub fn lode_status(s: &crate::TerraphonState, lode_id: &str) -> Result<String, String> {
    s.catalog.find(lode_id)
        .map(|l| l.to_sexp())
        .ok_or_else(|| format!("(:error \"lode not found: {}\")", sexp_escape(lode_id)))
}

pub fn register_lode(s: &mut crate::TerraphonState, lode: Lode) -> Result<String, String> {
    let id = lode.id.clone();
    s.catalog.register(lode);
    Ok(format!("(:ok :registered \"{}\")", sexp_escape(&id)))
}
