use super::{JsonSummaryReporter, Reporter};

pub fn registered_reporters() -> Vec<Box<dyn Reporter>> {
    vec![Box::new(JsonSummaryReporter::default())]
}
