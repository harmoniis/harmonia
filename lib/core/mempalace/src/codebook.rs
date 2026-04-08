use std::collections::HashMap;

pub struct AaakCodebook {
    entity_to_code: HashMap<String, String>,
    code_to_entity: HashMap<String, String>,
    next_index: u32,
}

impl AaakCodebook {
    pub fn new() -> Self {
        Self { entity_to_code: HashMap::new(), code_to_entity: HashMap::new(), next_index: 0 }
    }

    pub fn len(&self) -> usize { self.entity_to_code.len() }

    pub fn code_for(&mut self, entity: &str) -> String {
        let normalized = entity.to_lowercase().replace(' ', "-");
        if let Some(code) = self.entity_to_code.get(&normalized) {
            return code.clone();
        }
        let code = self.generate_code();
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
        let a = (n % 26) as u8;
        let b = ((n / 26) % 26) as u8;
        let c = ((n / 676) % 26) as u8;
        format!("{}{}{}", (b'A' + c) as char, (b'A' + b) as char, (b'A' + a) as char)
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
        assert_eq!(cb.code_for("memory-field"), "AAA");
        assert_eq!(cb.code_for("spectral"), "AAB");
        assert_eq!(cb.code_for("memory-field"), "AAA");
    }

    #[test]
    fn test_roundtrip() {
        let mut cb = AaakCodebook::new();
        cb.code_for("memory-field");
        cb.code_for("spectral");
        let json = cb.to_json();
        let cb2 = AaakCodebook::from_json(&json);
        assert_eq!(cb2.len(), 2);
        assert_eq!(cb2.lookup("AAA"), Some("memory-field".into()));
        assert_eq!(cb2.lookup("memory-field"), Some("AAA".into()));
    }
}
