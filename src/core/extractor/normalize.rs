use anyhow::Result;

use super::types::{
    ExtractorInput, NormalizedConversation, PreservedBlock, TermAnnotation, Utterance,
};

/// Static lookup table for normalizing key terms.
const TERM_MAP: &[(&str, &str)] = &[
    ("INとOUT", "interface_schema"),
    ("IN/OUT", "interface_schema"),
    ("処理している？", "processing_method_question"),
    ("パイプライン", "pipeline"),
    ("ステージ", "stage"),
    ("Situation Extractor", "situation_extractor"),
    ("Workflow Retriever", "workflow_retriever"),
    ("LLM", "llm_component"),
];

/// Normalize raw conversation text into structured utterances.
///
/// This is a synchronous, rule-based function — no LLM needed.
pub fn normalize(input: &ExtractorInput) -> Result<NormalizedConversation> {
    let text = input.conversation.trim();
    if text.is_empty() {
        return Ok(NormalizedConversation {
            utterances: Vec::new(),
            language: "en".to_string(),
        });
    }

    let language = detect_language(text);
    let raw_utterances = split_utterances(text);

    let mut utterances = Vec::with_capacity(raw_utterances.len());
    for (i, raw) in raw_utterances.into_iter().enumerate() {
        let (cleaned, preserved_blocks) = extract_preserved_blocks(&raw);
        let term_annotations = find_term_annotations(&cleaned);

        utterances.push(Utterance {
            index: i,
            text: cleaned,
            preserved_blocks,
            term_annotations,
        });
    }

    Ok(NormalizedConversation {
        utterances,
        language,
    })
}

/// Detect the primary language of the text.
///
/// Simple heuristic: if the text contains Hiragana or Katakana, it's Japanese.
fn detect_language(text: &str) -> String {
    for ch in text.chars() {
        let cp = ch as u32;
        // Hiragana: U+3040–U+309F, Katakana: U+30A0–U+30FF
        if (0x3040..=0x309F).contains(&cp) || (0x30A0..=0x30FF).contains(&cp) {
            return "ja".to_string();
        }
    }
    "en".to_string()
}

/// Split text into utterances.
///
/// Splits on double newlines first (paragraph boundaries), then on sentence
/// ending punctuation (。？！.?!). Preserves code blocks, quotes, and bullet
/// lists as single units.
fn split_utterances(text: &str) -> Vec<String> {
    let mut utterances = Vec::new();
    let mut current = String::new();
    let mut in_code_fence = false;

    for line in text.lines() {
        let trimmed = line.trim();

        // Track code fences
        if trimmed.starts_with("```") {
            in_code_fence = !in_code_fence;
            current.push_str(line);
            current.push('\n');
            // If we just closed a code fence, emit the block as one utterance
            if !in_code_fence {
                let utterance = current.trim().to_string();
                if !utterance.is_empty() {
                    utterances.push(utterance);
                }
                current.clear();
            }
            continue;
        }

        if in_code_fence {
            current.push_str(line);
            current.push('\n');
            continue;
        }

        // Quote lines (> prefix) — accumulate as part of current utterance
        // Bullet list lines (- or * prefix) — accumulate
        if trimmed.starts_with('>') || trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            current.push_str(line);
            current.push('\n');
            continue;
        }

        // Blank line = paragraph break
        if trimmed.is_empty() {
            if !current.trim().is_empty() {
                utterances.push(current.trim().to_string());
                current.clear();
            }
            continue;
        }

        // Regular line — append and check for sentence endings
        current.push_str(line);
        current.push('\n');

        // Split on sentence-ending punctuation
        let trimmed_current = current.trim();
        if ends_sentence(trimmed_current) {
            utterances.push(trimmed_current.to_string());
            current.clear();
        }
    }

    // Flush remaining
    if !current.trim().is_empty() {
        utterances.push(current.trim().to_string());
    }

    utterances
}

/// Check if the text ends with a sentence-ending character.
fn ends_sentence(text: &str) -> bool {
    let ch = text.chars().last().unwrap_or('\0');
    matches!(ch, '。' | '？' | '！' | '.' | '?' | '!')
}

/// Extract preserved structural blocks from raw text.
///
/// Returns (cleaned_text, preserved_blocks).
fn extract_preserved_blocks(raw: &str) -> (String, Vec<PreservedBlock>) {
    let mut blocks = Vec::new();
    let mut cleaned_lines = Vec::new();
    let mut in_code = false;
    let mut code_buf = String::new();
    let mut code_lang: Option<String> = None;
    let mut quote_buf = String::new();
    let mut list_items: Vec<String> = Vec::new();

    for line in raw.lines() {
        let trimmed = line.trim();

        // Code fence handling
        if trimmed.starts_with("```") {
            if in_code {
                // Close code block
                blocks.push(PreservedBlock::CodeBlock {
                    language: code_lang.take(),
                    content: code_buf.trim().to_string(),
                });
                code_buf.clear();
                in_code = false;
            } else {
                // Flush any accumulated quote or list
                flush_quote(&mut quote_buf, &mut blocks);
                flush_list(&mut list_items, &mut blocks);

                // Open code block
                in_code = true;
                // Extract language hint from ```lang
                let lang = trimmed.strip_prefix("```").unwrap_or("").trim();
                if !lang.is_empty() {
                    code_lang = Some(lang.to_string());
                }
            }
            continue;
        }

        if in_code {
            code_buf.push_str(line);
            code_buf.push('\n');
            continue;
        }

        // Quote line
        if let Some(rest) = trimmed.strip_prefix("> ") {
            flush_list(&mut list_items, &mut blocks);
            quote_buf.push_str(rest);
            quote_buf.push('\n');
            continue;
        }

        // Bullet list line
        if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            flush_quote(&mut quote_buf, &mut blocks);
            list_items.push(rest.to_string());
            continue;
        }

        // Regular line — flush any accumulated blocks
        flush_quote(&mut quote_buf, &mut blocks);
        flush_list(&mut list_items, &mut blocks);
        cleaned_lines.push(line);
    }

    // Flush remaining
    flush_quote(&mut quote_buf, &mut blocks);
    flush_list(&mut list_items, &mut blocks);

    let cleaned = if cleaned_lines.is_empty() && !blocks.is_empty() {
        // Text was entirely preserved blocks
        String::new()
    } else {
        cleaned_lines.join("\n").trim().to_string()
    };

    (cleaned, blocks)
}

fn flush_quote(buf: &mut String, blocks: &mut Vec<PreservedBlock>) {
    if !buf.is_empty() {
        blocks.push(PreservedBlock::Quote {
            content: buf.trim().to_string(),
        });
        buf.clear();
    }
}

fn flush_list(items: &mut Vec<String>, blocks: &mut Vec<PreservedBlock>) {
    if !items.is_empty() {
        blocks.push(PreservedBlock::BulletList {
            items: std::mem::take(items),
        });
    }
}

/// Find normalized term annotations in the text.
fn find_term_annotations(text: &str) -> Vec<TermAnnotation> {
    let mut annotations = Vec::new();
    for (original, normalized) in TERM_MAP {
        if text.contains(original) {
            annotations.push(TermAnnotation {
                original: (*original).to_string(),
                normalized: (*normalized).to_string(),
            });
        }
    }
    annotations
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language_japanese() {
        assert_eq!(detect_language("こんにちは世界"), "ja");
        assert_eq!(detect_language("パイプライン"), "ja");
    }

    #[test]
    fn test_detect_language_english() {
        assert_eq!(detect_language("hello world"), "en");
    }

    #[test]
    fn test_detect_language_mixed() {
        // Japanese character present → "ja"
        assert_eq!(detect_language("hello こんにちは"), "ja");
    }

    #[test]
    fn test_split_japanese_utterances() {
        let text = "これは一文です。\n\nこれも一文です。";
        let utterances = split_utterances(text);
        assert_eq!(utterances.len(), 2);
        assert_eq!(utterances[0], "これは一文です。");
        assert_eq!(utterances[1], "これも一文です。");
    }

    #[test]
    fn test_split_paragraph_break() {
        let text = "First paragraph.\n\nSecond paragraph.";
        let utterances = split_utterances(text);
        assert_eq!(utterances.len(), 2);
    }

    #[test]
    fn test_preserve_code_block() {
        let input = ExtractorInput {
            conversation: "Here is code:\n```rust\nfn main() {}\n```\nDone.".to_string(),
            context: vec![],
        };
        let result = normalize(&input).unwrap();
        // Should have utterances, one of which contains a CodeBlock
        let has_code = result.utterances.iter().any(|u| {
            u.preserved_blocks
                .iter()
                .any(|b| matches!(b, PreservedBlock::CodeBlock { .. }))
        });
        assert!(has_code, "should preserve code block");
    }

    #[test]
    fn test_preserve_bullet_list() {
        let input = ExtractorInput {
            conversation: "Items:\n- first\n- second\nDone.".to_string(),
            context: vec![],
        };
        let result = normalize(&input).unwrap();
        let has_list = result.utterances.iter().any(|u| {
            u.preserved_blocks
                .iter()
                .any(|b| matches!(b, PreservedBlock::BulletList { .. }))
        });
        assert!(has_list, "should preserve bullet list");
    }

    #[test]
    fn test_normalize_key_terms() {
        let input = ExtractorInput {
            conversation: "INとOUTを定義する".to_string(),
            context: vec![],
        };
        let result = normalize(&input).unwrap();
        let has_annotation = result.utterances.iter().any(|u| {
            u.term_annotations
                .iter()
                .any(|a| a.normalized == "interface_schema")
        });
        assert!(
            has_annotation,
            "should normalize 'INとOUT' to 'interface_schema'"
        );
    }

    #[test]
    fn test_empty_input() {
        let input = ExtractorInput {
            conversation: "".to_string(),
            context: vec![],
        };
        let result = normalize(&input).unwrap();
        assert!(result.utterances.is_empty());
    }
}
