//! ToolRAG retrieval engine — dependency-free, keyword (BM25) based.
//!
//! Mirrors TinyAgent's ToolRAG pattern: per query, retrieve the small subset
//! of "tools" (here: skills) that are actually relevant, so the model never
//! has to reason over the full catalog. Unlike TinyAgent we use BM25 instead
//! of embeddings to keep the dependency list empty and latency predictable.
//!
//! Each document is modelled as two fields — a short `name` and a longer
//! `description` — and a query is scored as
//! `W_NAME * bm25(name) + W_DESC * bm25(description)`, so a hit on the name
//! counts for more than a hit buried in the description.

use std::collections::{HashMap, HashSet};

/// BM25 term-frequency saturation parameter (standard default).
const K1: f64 = 1.2;
/// BM25 length-normalization parameter (standard default).
const B: f64 = 0.75;
/// Score weight for the `name` field.
const W_NAME: f64 = 2.0;
/// Score weight for the `description` field.
const W_DESC: f64 = 1.0;
/// Index of the `name` field within the per-document field arrays.
const FIELD_NAME: usize = 0;
/// Index of the `description` field within the per-document field arrays.
const FIELD_DESC: usize = 1;

/// A small English stopword set, dropped during tokenization so frequent
/// function words don't dominate the score.
const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "for", "from", "has", "in", "is", "it", "its",
    "of", "on", "or", "that", "the", "this", "to", "use", "was", "were", "will", "with",
];

/// A retrieval hit. `index` is the position of the matched document in the
/// slice passed to [`Retriever::from_docs`].
#[derive(Debug, Clone, PartialEq)]
pub struct RankedDoc {
    pub index: usize,
    pub score: f64,
}

/// A multi-field BM25 index over a fixed set of `(name, description)` docs.
pub struct Retriever {
    /// Per-document, per-field term frequencies: `tf[doc][field][term]`.
    tf: Vec<[HashMap<String, u32>; 2]>,
    /// Token length of each field for each document.
    field_len: Vec<[usize; 2]>,
    /// Average field length across the corpus, per field.
    avg_field_len: [f64; 2],
    /// Number of documents containing each term (in either field).
    doc_freq: HashMap<String, usize>,
    num_docs: usize,
}

impl Retriever {
    /// Build an index from `(name, description)` document pairs.
    pub fn from_docs(docs: &[(&str, &str)]) -> Self {
        let n = docs.len();
        let mut tf: Vec<[HashMap<String, u32>; 2]> = Vec::with_capacity(n);
        let mut field_len: Vec<[usize; 2]> = Vec::with_capacity(n);
        let mut doc_freq: HashMap<String, usize> = HashMap::new();
        let mut sum_len = [0usize; 2];

        for (name, desc) in docs {
            let name_tokens = tokenize(name);
            let desc_tokens = tokenize(desc);
            field_len.push([name_tokens.len(), desc_tokens.len()]);
            sum_len[0] += name_tokens.len();
            sum_len[1] += desc_tokens.len();

            let name_tf = term_counts(&name_tokens);
            let desc_tf = term_counts(&desc_tokens);

            // A term contributes to doc-frequency if it appears in either field.
            let mut seen: HashSet<&str> = HashSet::new();
            for t in name_tf.keys() {
                seen.insert(t.as_str());
            }
            for t in desc_tf.keys() {
                seen.insert(t.as_str());
            }
            for t in seen {
                *doc_freq.entry(t.to_string()).or_default() += 1;
            }

            tf.push([name_tf, desc_tf]);
        }

        let avg_field_len = [avg_or_zero(sum_len[0], n), avg_or_zero(sum_len[1], n)];

        Self {
            tf,
            field_len,
            avg_field_len,
            doc_freq,
            num_docs: n,
        }
    }

    /// Rank documents against `query`, returning up to `top_k` hits with a
    /// positive score, best first. Ties break by document index (stable).
    pub fn search(&self, query: &str, top_k: usize) -> Vec<RankedDoc> {
        let q_terms = unique_terms(query);
        if q_terms.is_empty() || self.num_docs == 0 {
            return Vec::new();
        }

        let n = self.num_docs as f64;
        let mut hits: Vec<RankedDoc> = Vec::new();

        for doc in 0..self.num_docs {
            let mut total = 0.0;
            for (field, weight) in [(FIELD_NAME, W_NAME), (FIELD_DESC, W_DESC)] {
                let avg = self.avg_field_len[field];
                if avg == 0.0 {
                    continue;
                }
                let len = self.field_len[doc][field] as f64;
                let norm = 1.0 - B + B * (len / avg);
                let tf_map = &self.tf[doc][field];

                let mut field_score = 0.0;
                for term in &q_terms {
                    let Some(&tfreq) = tf_map.get(term) else {
                        continue;
                    };
                    let df = *self.doc_freq.get(term).unwrap_or(&0) as f64;
                    if df == 0.0 {
                        continue;
                    }
                    // Lucene-style IDF: always positive.
                    let idf = (1.0 + (n - df + 0.5) / (df + 0.5)).ln();
                    let tf64 = tfreq as f64;
                    field_score += idf * (tf64 * (K1 + 1.0)) / (tf64 + K1 * norm);
                }
                total += weight * field_score;
            }

            if total > 0.0 {
                hits.push(RankedDoc {
                    index: doc,
                    score: total,
                });
            }
        }

        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.index.cmp(&b.index))
        });
        hits.truncate(top_k);
        hits
    }
}

fn avg_or_zero(sum: usize, n: usize) -> f64 {
    if n == 0 { 0.0 } else { sum as f64 / n as f64 }
}

/// Lowercase, split on non-alphanumerics, drop stopwords and tokens shorter
/// than 2 characters.
fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.len() >= 2 && !STOPWORDS.contains(s))
        .map(String::from)
        .collect()
}

/// Term -> frequency map for a token list.
fn term_counts(tokens: &[String]) -> HashMap<String, u32> {
    let mut counts = HashMap::new();
    for tok in tokens {
        *counts.entry(tok.clone()).or_default() += 1;
    }
    counts
}

/// Deduplicated, tokenized query terms (preserving first-seen order).
fn unique_terms(query: &str) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    tokenize(query)
        .into_iter()
        .filter(|t| seen.insert(t.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retrieves_relevant_doc_and_filters_irrelevant() {
        let docs = [
            ("math-evaluator", "evaluate math expressions and arithmetic"),
            ("translator", "translate text between languages"),
        ];
        let r = Retriever::from_docs(&docs);

        let hits = r.search("evaluate math", 5);
        assert_eq!(hits.len(), 1, "only the math doc should match");
        assert_eq!(hits[0].index, 0);
        assert!(hits[0].score > 0.0);
    }

    #[test]
    fn test_name_match_outweighs_description_match() {
        // "math" appears in doc0's name and doc1's description.
        let docs = [
            ("math", "something unrelated entirely"),
            ("other", "about math and more"),
        ];
        let r = Retriever::from_docs(&docs);

        let hits = r.search("math", 5);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].index, 0, "name match should rank first");
        assert!(hits[0].score > hits[1].score);
    }

    #[test]
    fn test_empty_query_returns_empty() {
        let docs = [("calculator", "evaluate math expressions")];
        let r = Retriever::from_docs(&docs);
        assert!(r.search("", 5).is_empty());
        assert!(
            r.search("the a an of", 5).is_empty(),
            "stopwords-only query"
        );
    }

    #[test]
    fn test_no_matches_returns_empty() {
        let docs = [("calculator", "evaluate math expressions")];
        let r = Retriever::from_docs(&docs);
        assert!(r.search("translate spanish", 5).is_empty());
    }

    #[test]
    fn test_respects_top_k() {
        let docs = [
            ("math", "math math math"),
            ("math-two", "math math"),
            ("math-three", "math"),
        ];
        let r = Retriever::from_docs(&docs);
        let hits = r.search("math", 2);
        assert!(hits.len() <= 2);
    }

    #[test]
    fn test_empty_corpus_returns_empty() {
        let r = Retriever::from_docs(&[]);
        assert!(r.search("anything", 5).is_empty());
    }

    #[test]
    fn test_dedupes_repeated_query_terms() {
        let docs = [("calculator", "evaluate math expressions")];
        let r = Retriever::from_docs(&docs);
        // "math math math" should behave like a single "math".
        let one = r.search("math", 5);
        let three = r.search("math math math", 5);
        assert_eq!(one.len(), three.len());
        assert!((one[0].score - three[0].score).abs() < 1e-9);
    }
}
