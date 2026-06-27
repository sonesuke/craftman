pub mod app;
pub mod backends;
pub mod core;
pub mod tools;

// ---------------------------------------------------------------------------
// Convenience re-exports
// ---------------------------------------------------------------------------

pub use backends::MockBackend;
pub use backends::OllamaBackend;
pub use core::harness::{EventSink, Harness, HarnessEvent};
pub use core::llm::EmbeddingModel;
pub use core::llm::ResponseModel;
