//! Structured logging of harness events as JSONL.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use anyhow::Result;

use crate::core::harness::{EventSink, HarnessEvent};

/// [`EventSink`] that appends each structured event as one JSON line.
///
/// High-volume deltas (reasoning/assistant text) are skipped to keep the log
/// focused on the meaningful steps: retrieval, activation, tool calls and
/// results, turn completion (with token usage).
pub struct JsonlLogger {
    file: std::fs::File,
}

impl JsonlLogger {
    pub fn new(path: &Path) -> Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self { file })
    }
}

impl EventSink for JsonlLogger {
    fn on_event(&mut self, event: &HarnessEvent) {
        match event {
            HarnessEvent::ReasoningDelta { .. } | HarnessEvent::AssistantDelta { .. } => return,
            _ => {}
        }
        if let Ok(line) = serde_json::to_string(event) {
            let _ = writeln!(self.file, "{line}");
        }
    }
}
