pub fn chunk_content(content: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    let chunk_size = chunk_size.max(100);
    let overlap = overlap.min(chunk_size / 2);
    if content.len() <= chunk_size {
        return vec![content.to_string()];
    }
    if content.contains("\n\n") {
        chunk_by_paragraphs(content, chunk_size, overlap)
    } else {
        chunk_by_sentences(content, chunk_size, overlap)
    }
}

fn chunk_by_paragraphs(content: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    let paragraphs: Vec<&str> = content.split("\n\n").collect();
    let (mut chunks, remainder) = paragraphs.iter()
        .fold((Vec::new(), String::new()), |(mut chunks, mut current), para| {
            if current.len() + para.len() + 2 > chunk_size && !current.is_empty() {
                let keep_from = current.len().saturating_sub(overlap);
                let overlap_text = current[keep_from..].to_string();
                chunks.push(current);
                current = overlap_text;
            }
            if !current.is_empty() { current.push_str("\n\n"); }
            current.push_str(para);
            (chunks, current)
        });
    if !remainder.is_empty() { chunks.push(remainder); }
    chunks
}

fn chunk_by_sentences(content: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < content.len() {
        let end = (start + chunk_size).min(content.len());
        let boundary = if end < content.len() {
            content[start..end]
                .rfind(". ")
                .or_else(|| content[start..end].rfind(".\n"))
                .or_else(|| content[start..end].rfind('\n'))
                .map(|pos| start + pos + 1)
                .unwrap_or(end)
        } else {
            end
        };
        chunks.push(content[start..boundary].to_string());
        start = boundary.saturating_sub(overlap);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_content() {
        let chunks = chunk_content("hello world", 800, 100);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "hello world");
    }

    #[test]
    fn test_paragraph_split() {
        let content = "First paragraph with enough text to exceed the minimum.\n\n\
            Second paragraph also has enough content to matter here.\n\n\
            Third paragraph fills out the rest of the required length.";
        let chunks = chunk_content(content, 100, 10);
        assert!(chunks.len() >= 2);
    }
}
