use harmonia_actor_protocol::MemoryError;

use crate::sexp_escape;

#[derive(Clone, Debug)]
pub struct AaakEntry {
    pub entities: Vec<(String, String, u32)>,
    pub topic: String,
    pub weight: f64,
    pub flags: Vec<String>,
    pub source_drawer_ids: Vec<u64>,
}

impl AaakEntry {
    pub fn to_sexp(&self) -> String {
        let entities_sexp: Vec<String> = self.entities.iter()
            .map(|(code, name, count)| format!("(\"{}\" \"{}\" {})", code, sexp_escape(name), count))
            .collect();
        let flags_sexp: Vec<String> = self.flags.iter().map(|f| format!(":{}", f)).collect();
        let ids_sexp: Vec<String> = self.source_drawer_ids.iter().map(|id| id.to_string()).collect();
        format!(
            "(:aaak :entities ({}) :topic \"{}\" :weight {:.2} :flags ({}) :sources ({}))",
            entities_sexp.join(" "), sexp_escape(&self.topic), self.weight, flags_sexp.join(" "), ids_sexp.join(" "),
        )
    }
}

pub fn compress_aaak(
    s: &mut crate::PalaceState,
    drawer_ids: &[u64],
) -> Result<String, MemoryError> {
    if drawer_ids.is_empty() {
        return Err(MemoryError::InvalidContent("no drawer ids provided".into()));
    }
    let drawers = s.drawers.get_by_ids(drawer_ids);
    if drawers.is_empty() {
        return Err(MemoryError::DrawerNotFound(drawer_ids[0]));
    }
    let combined = combine_content(&drawers);
    let word_counts = count_words(&combined);
    let top_entities = select_top_entities(word_counts, 15);
    let entities = assign_codes(&mut s.codebook, top_entities);
    let topic = extract_topic(&drawers);
    let weight = combined.len() as f64 / 100.0;
    let entry = AaakEntry {
        entities,
        topic,
        weight,
        flags: vec!["compressed".into()],
        source_drawer_ids: drawer_ids.to_vec(),
    };
    Ok(entry.to_sexp())
}

pub fn codebook_lookup(
    s: &crate::PalaceState,
    code_or_entity: &str,
) -> Result<String, MemoryError> {
    match s.codebook.lookup(code_or_entity) {
        Some(result) => Ok(format!(
            "(:ok :input \"{}\" :result \"{}\")",
            sexp_escape(code_or_entity), sexp_escape(&result)
        )),
        None => Err(MemoryError::NodeNotFound(code_or_entity.into())),
    }
}

// ── Helpers ──

fn combine_content(drawers: &[&crate::drawer::Drawer]) -> String {
    drawers.iter().map(|d| d.content.as_str()).collect::<Vec<_>>().join(" ")
}

fn count_words(text: &str) -> Vec<(String, u32)> {
    let counts: std::collections::HashMap<String, u32> = extract_words(text)
        .into_iter()
        .fold(std::collections::HashMap::new(), |mut acc, w| {
            *acc.entry(w).or_insert(0) += 1;
            acc
        });
    counts.into_iter().collect()
}

fn select_top_entities(mut word_counts: Vec<(String, u32)>, max: usize) -> Vec<(String, u32)> {
    word_counts.retain(|(w, c)| *c >= 2 && !is_stop_word(w) && w.len() > 2);
    word_counts.sort_by(|a, b| b.1.cmp(&a.1));
    word_counts.truncate(max);
    word_counts
}

fn assign_codes(codebook: &mut crate::codebook::AaakCodebook, entities: Vec<(String, u32)>) -> Vec<(String, String, u32)> {
    entities.into_iter()
        .map(|(word, count)| {
            let code = codebook.code_for(&word);
            (code, word, count)
        })
        .collect()
}

fn extract_topic(drawers: &[&crate::drawer::Drawer]) -> String {
    drawers.first()
        .and_then(|d| d.tags.first().cloned())
        .unwrap_or_else(|| "unknown".into())
}

fn extract_words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect()
}

const STOP_WORDS: &[&str] = &[
    "the","a","an","is","are","was","were","be","been","being","have","has","had",
    "do","does","did","will","would","could","should","may","might","shall","can",
    "to","of","in","for","on","with","at","by","from","as","into","through","during",
    "before","after","above","below","between","and","but","or","nor","not","so","yet",
    "both","either","neither","each","every","all","any","few","more","most","other",
    "some","such","no","only","own","same","than","too","very","just","because",
    "this","that","these","those","it","its",
];

fn is_stop_word(word: &str) -> bool {
    STOP_WORDS.contains(&word)
}
