//! Pattern detection and template matching.

mod lcs;
mod template;
mod tokenizer;
mod types;

pub use template::detect_templates;
pub use tokenizer::tokenize;
pub use types::infer_variable_type;

