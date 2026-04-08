use std::collections::HashMap;

const MAX_CODEBOOK_ENTRIES: usize = 256;

pub struct AaakCodebook {
    entity_to_code: HashMap<String, String>,
    code_to_entity: HashMap<String, String>,
    next_index: u32,
    /// Track access order for LRU eviction: entity -> last access counter
    access_order: HashMap<String, u64>,
    access_counter: u64,
}

impl AaakCodebook {
    pub fn new() -> Self {
        Self {
            entity_to_code: HashMap::new(),
            code_to_entity: HashMap::new(),
            next_index: 0,
            access_order: HashMap::new(),
            access_counter: 0,
        }
    }

    pub fn len(&self) -> usize { self.entity_to_code.len() }

    pub fn code_for(&mut self, entity: &str) -> String {
        let normalized = entity.to_lowercase().replace(' ', "-");
        if let Some(code) = self.entity_to_code.get(&normalized) {
            // Update access order for LRU tracking.
            self.access_counter += 1;
            self.access_order.insert(normalized, self.access_counter);
            return code.clone();
        }
        // Evict LRU entry if at capacity.
        if self.entity_to_code.len() >= MAX_CODEBOOK_ENTRIES {
            self.evict_lru();
        }
        let code = self.generate_code();
        self.access_counter += 1;
        self.access_order.insert(normalized.clone(), self.access_counter);
        self.entity_to_code.insert(normalized.clone(), code.clone());
        self.code_to_entity.insert(code.clone(), normalized);
        code
    }

    pub fn entity_for(&self, code: &str) -> Option<&str> {
        self.code_to_entity.get(code).map(|s| s.as_str())
    }

    pub fn lookup(&self, entity_or_code: &str) -> Option<String> {
        if let Some(entity) = self.code_to_entity.get(entity_or_code) {
            Some(entity.clone())
        } else {
            let normalized = entity_or_code.to_lowercase().replace(' ', "-");
            self.entity_to_code.get(&normalized).cloned()
        }
    }

    fn generate_code(&mut self) -> String {
        let n = self.next_index;
        self.next_index += 1;
        let index = n as usize;
        if index < 26 {
            // High-frequency: single uppercase letter
            format!("{}", (b'A' + index as u8) as char)
        } else if index < 702 {
            // Medium-frequency: two lowercase letters
            let m = index - 26;
            format!("{}{}", (b'a' + (m / 26) as u8) as char, (b'a' + (m % 26) as u8) as char)
        } else {
            // Low-frequency: three uppercase letters (original behavior)
            let m = index - 702;
            let a = (m % 26) as u8;
            let b = ((m / 26) % 26) as u8;
            let c = ((m / 676) % 26) as u8;
            format!("{}{}{}", (b'A' + c) as char, (b'A' + b) as char, (b'A' + a) as char)
        }
    }

    fn evict_lru(&mut self) {
        // Find the entity with the oldest (lowest) access counter.
        let lru_entity = self.access_order.iter()
            .min_by_key(|(_, &counter)| counter)
            .map(|(entity, _)| entity.clone());
        if let Some(entity) = lru_entity {
            if let Some(code) = self.entity_to_code.remove(&entity) {
                self.code_to_entity.remove(&code);
            }
            self.access_order.remove(&entity);
        }
    }

    pub fn to_json(&self) -> String {
        let entries: Vec<serde_json::Value> = self.entity_to_code.iter()
            .map(|(entity, code)| serde_json::json!([entity, code]))
            .collect();
        serde_json::json!({ "entries": entries, "next": self.next_index }).to_string()
    }

    pub fn from_json(json: &str) -> Self {
        let mut cb = Self::new();
        let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else { return cb; };
        if let Some(n) = v.get("next").and_then(|n| n.as_u64()) {
            cb.next_index = n as u32;
        }
        if let Some(entries) = v.get("entries").and_then(|e| e.as_array()) {
            for pair in entries {
                if let Some(arr) = pair.as_array() {
                    if let [entity, code] = arr.as_slice() {
                        if let (Some(e), Some(c)) = (entity.as_str(), code.as_str()) {
                            cb.access_counter += 1;
                            cb.access_order.insert(e.to_string(), cb.access_counter);
                            cb.entity_to_code.insert(e.to_string(), c.to_string());
                            cb.code_to_entity.insert(c.to_string(), e.to_string());
                        }
                    }
                }
            }
        }
        cb
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_generation() {
        let mut cb = AaakCodebook::new();
        // First 26 are single uppercase letters (high-frequency).
        assert_eq!(cb.code_for("memory-field"), "A");
        assert_eq!(cb.code_for("spectral"), "B");
        assert_eq!(cb.code_for("memory-field"), "A"); // deduplicated
    }

    #[test]
    fn test_frequency_adaptive_codes() {
        let mut cb = AaakCodebook::new();
        // Fill all 26 single-letter codes.
        for i in 0..26 {
            cb.code_for(&format!("entity-{}", i));
        }
        assert_eq!(cb.len(), 26);
        // Next code should be two lowercase letters.
        let code = cb.code_for("entity-26");
        assert_eq!(code, "aa");
        let code = cb.code_for("entity-27");
        assert_eq!(code, "ab");
    }

    #[test]
    fn test_roundtrip() {
        let mut cb = AaakCodebook::new();
        cb.code_for("memory-field");
        cb.code_for("spectral");
        let json = cb.to_json();
        let cb2 = AaakCodebook::from_json(&json);
        assert_eq!(cb2.len(), 2);
        assert_eq!(cb2.lookup("A"), Some("memory-field".into()));
        assert_eq!(cb2.lookup("memory-field"), Some("A".into()));
    }

    #[test]
    fn test_lru_eviction() {
        let mut cb = AaakCodebook::new();
        // Fill to capacity.
        for i in 0..MAX_CODEBOOK_ENTRIES {
            cb.code_for(&format!("entity-{}", i));
        }
        assert_eq!(cb.len(), MAX_CODEBOOK_ENTRIES);
        // Access entity-0 to make it recent.
        cb.code_for("entity-0");
        // Adding a new entity should evict the LRU (entity-1, since entity-0 was just accessed).
        cb.code_for("brand-new-entity");
        assert_eq!(cb.len(), MAX_CODEBOOK_ENTRIES);
        // entity-1 should have been evicted (it was the LRU).
        assert!(cb.lookup("entity-1").is_none());
        // entity-0 and brand-new-entity should still be present.
        assert!(cb.lookup("entity-0").is_some());
        assert!(cb.lookup("brand-new-entity").is_some());
    }
}
