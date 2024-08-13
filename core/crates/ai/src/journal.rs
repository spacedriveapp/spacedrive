use crate::{concept::*, define_concept, Prompt};
use chrono::prelude::*;
use std::any::Any;

// The Journal allows the model to recall extremely important highly summarized memories and information as sort of a guidebook. It will constantly review and audit this information as it processes new information and experiences. The Journal is a critical component of the AI's ability to learn and adapt. It is formatted in markdown and is ingested into a vector database for querying.

#[derive(Debug, Clone, Prompt, Default)]
#[prompt(instruct = r###"
		Your journal is for recording important information that should be reviewed and audited regularly. This information should be highly summarized and formatted in markdown. Always check the journal thoroughly before adding new entries to ensure that the information is accurate and relevant. Your journal is your bible, treat it as such. Keep it structured and organized and review it often.
	"###)]
pub struct Journal {
	pub entries: Vec<JournalEntry>,
}
define_concept!(Journal);

#[derive(Debug, Clone, Prompt, Default)]
pub struct JournalEntry {
	id: i64,
	title: String,
	#[prompt(instruct = "The content of this journal entry in Markdown format.")]
	content: String,
	timestamp: DateTime<Utc>,
	#[prompt(instruct = "On a scale of 1-100, how relevant is this entry?")]
	relevance: u16,
	#[prompt(instruct = "Choose 3-5 tags that best describe this entry.")]
	tags: Vec<String>,
}
define_concept!(JournalEntry);

// #[derive(Debug, Clone, Prompt)]
// pub struct Chapter {
// 	id: i64,
// 	title: String,
// 	entries: Vec<JournalEntry>,
// 	timestamp: DateTime<Utc>,
// 	relevance: u16,    // How relevant this chapter is (1-100)
// 	tags: Vec<String>, // Tags for easy searching and categorization
// }
