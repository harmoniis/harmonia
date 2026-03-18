use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Clone, Copy)]
pub enum VoiceKind {
    SpeechToText,
    TextToSpeech,
}

impl VoiceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            VoiceKind::SpeechToText => "stt",
            VoiceKind::TextToSpeech => "tts",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct VoiceOffering {
    pub id: &'static str,
    pub provider: &'static str,
    pub kind: VoiceKind,
    pub quality: u8,
    pub speed: u8,
    pub tags: &'static [&'static str],
}

static ROUND_ROBIN: AtomicUsize = AtomicUsize::new(0);

pub fn select_from_pool(offerings: &[VoiceOffering], kind: VoiceKind) -> String {
    let filtered: Vec<&VoiceOffering> = offerings
        .iter()
        .filter(|o| {
            matches!(
                (&o.kind, &kind),
                (VoiceKind::SpeechToText, VoiceKind::SpeechToText)
                    | (VoiceKind::TextToSpeech, VoiceKind::TextToSpeech)
            )
        })
        .collect();

    if filtered.is_empty() {
        return offerings
            .first()
            .map(|o| o.id.to_string())
            .unwrap_or_default();
    }

    let idx = ROUND_ROBIN.fetch_add(1, Ordering::Relaxed) % filtered.len();
    filtered[idx].id.to_string()
}

pub fn pool_fallbacks(offerings: &[VoiceOffering], selected: &str) -> Vec<String> {
    offerings
        .iter()
        .filter(|o| o.id != selected)
        .map(|o| o.id.to_string())
        .collect()
}

pub fn offerings_to_sexp(offerings: &[VoiceOffering]) -> String {
    if offerings.is_empty() {
        return "nil".to_string();
    }
    let items: Vec<String> = offerings
        .iter()
        .map(|o| {
            format!(
                "(:id \"{}\" :provider \"{}\" :kind \"{}\" :quality {} :speed {} :tags ({}))",
                o.id,
                o.provider,
                o.kind.as_str(),
                o.quality,
                o.speed,
                o.tags
                    .iter()
                    .map(|t| format!("\"{}\"", t))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        })
        .collect();
    format!("({})", items.join(" "))
}

pub fn strip_provider_prefix(model: &str) -> &str {
    model.split_once('/').map(|(_, m)| m).unwrap_or(model)
}
