pub mod app;
pub mod backends;
pub mod core;

// ---------------------------------------------------------------------------
// Convenience re-exports
// ---------------------------------------------------------------------------

pub use app::AssistantService;
pub use backends::MockBackend;
pub use backends::OllamaBackend;
pub use core::llm::EmbeddingModel;
pub use core::llm::ResponseModel;
