//! Batch rename rules: an ordered list applied to each name.
//!
//! Each rule takes the name as the rules before it left it. Text a rule
//! introduces can hold tokens, expanded as the rule applies: `{name}` and
//! `{ext}` for the stem and the extension with its dot as they stand,
//! `{parent}` for the directory's name, `{n}` or `{n:04}` for the counter
//! of the target's position, `{date:%Y-%m-%d}` for the modification time,
//! and `{captured:%Y-%m-%d}` for the capture time the store holds, or the
//! modification time where it holds none.

use std::path::Path;

use chrono::{DateTime, Local};
use regex::Regex;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RenameRule {
	/// Replace `find` with `with`, in the stem or the whole name.
	Replace {
		find: String,
		with: String,
		#[serde(default)]
		regex: bool,
		#[serde(default)]
		whole_name: bool,
	},
	Case {
		stem: CaseRule,
		#[serde(default)]
		extension: ExtensionCase,
	},
	Affix {
		#[serde(default)]
		prefix: String,
		#[serde(default)]
		suffix: String,
	},
	/// The stem from a pattern holding `{n}`, counted from `start` in
	/// steps of `step` over the targets in order.
	Sequence {
		pattern: String,
		#[serde(default = "one")]
		start: u64,
		#[serde(default = "one")]
		step: u64,
	},
	/// The whole name from a pattern.
	Template { pattern: String },
}

fn one() -> u64 {
	1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum CaseRule {
	Lower,
	Upper,
	Title,
	Keep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionCase {
	Lower,
	#[default]
	Keep,
}

/// What the tokens of one target expand to.
#[derive(Debug, Clone)]
pub struct Target {
	pub name: String,
	pub parent: String,
	/// The target's position among the targets, from 0.
	pub index: u64,
	pub modified: DateTime<Local>,
	pub captured: Option<DateTime<Local>>,
}

/// The name as the rules leave it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Name {
	stem: String,
	/// Without its dot.
	extension: Option<String>,
}

impl Name {
	fn parse(name: &str) -> Self {
		let path = Path::new(name);
		let stem = path
			.file_stem()
			.and_then(|stem| stem.to_str())
			.unwrap_or(name)
			.to_string();
		let extension = path
			.extension()
			.and_then(|extension| extension.to_str())
			.map(str::to_string);
		Self { stem, extension }
	}

	fn whole(&self) -> String {
		match &self.extension {
			Some(extension) => format!("{}.{extension}", self.stem),
			None => self.stem.clone(),
		}
	}

	fn dotted_extension(&self) -> String {
		self.extension
			.as_ref()
			.map(|extension| format!(".{extension}"))
			.unwrap_or_default()
	}
}

/// A rule that cannot apply: a regex that does not parse.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct RuleError(pub String);

/// Check every rule before any name is computed.
pub fn check_rules(rules: &[RenameRule]) -> Result<(), RuleError> {
	for rule in rules {
		if let RenameRule::Replace {
			find, regex: true, ..
		} = rule
		{
			Regex::new(find)
				.map_err(|error| RuleError(format!("the pattern {find:?}: {error}")))?;
		}
		if let RenameRule::Sequence { step: 0, .. } = rule {
			return Err(RuleError("a sequence cannot step by 0".to_string()));
		}
	}
	Ok(())
}

/// The name the rules give one target.
pub fn apply(rules: &[RenameRule], target: &Target) -> Result<String, RuleError> {
	let mut name = Name::parse(&target.name);
	let mut counter = target.index + 1;
	for rule in rules {
		match rule {
			RenameRule::Replace {
				find,
				with,
				regex,
				whole_name,
			} => {
				let with = expand(with, &name, target, counter);
				let subject = if *whole_name {
					name.whole()
				} else {
					name.stem.clone()
				};
				let replaced = if *regex {
					let pattern = Regex::new(find)
						.map_err(|error| RuleError(format!("the pattern {find:?}: {error}")))?;
					pattern.replace_all(&subject, with.as_str()).into_owned()
				} else if find.is_empty() {
					subject
				} else {
					subject.replace(find.as_str(), &with)
				};
				if *whole_name {
					name = Name::parse(&replaced);
				} else {
					name.stem = replaced;
				}
			}
			RenameRule::Case { stem, extension } => {
				name.stem = match stem {
					CaseRule::Lower => name.stem.to_lowercase(),
					CaseRule::Upper => name.stem.to_uppercase(),
					CaseRule::Title => title_case(&name.stem),
					CaseRule::Keep => name.stem,
				};
				if *extension == ExtensionCase::Lower {
					name.extension = name.extension.map(|extension| extension.to_lowercase());
				}
			}
			RenameRule::Affix { prefix, suffix } => {
				let prefix = expand(prefix, &name, target, counter);
				let suffix = expand(suffix, &name, target, counter);
				name.stem = format!("{prefix}{}{suffix}", name.stem);
			}
			RenameRule::Sequence {
				pattern,
				start,
				step,
			} => {
				counter = start + target.index * step;
				name.stem = expand(pattern, &name, target, counter);
			}
			RenameRule::Template { pattern } => {
				let whole = expand(pattern, &name, target, counter);
				name = Name::parse(&whole);
			}
		}
	}
	Ok(name.whole())
}

/// Expand the tokens in `text` for the name as it stands.
fn expand(text: &str, name: &Name, target: &Target, counter: u64) -> String {
	let mut out = String::with_capacity(text.len());
	let mut rest = text;
	while let Some(open) = rest.find('{') {
		out.push_str(&rest[..open]);
		let Some(close) = rest[open..].find('}') else {
			out.push_str(&rest[open..]);
			return out;
		};
		let token = &rest[open + 1..open + close];
		match token_value(token, name, target, counter) {
			Some(value) => out.push_str(&value),
			None => out.push_str(&rest[open..open + close + 1]),
		}
		rest = &rest[open + close + 1..];
	}
	out.push_str(rest);
	out
}

fn token_value(token: &str, name: &Name, target: &Target, counter: u64) -> Option<String> {
	if token == "name" {
		return Some(name.stem.clone());
	}
	if token == "ext" {
		return Some(name.dotted_extension());
	}
	if token == "parent" {
		return Some(target.parent.clone());
	}
	if token == "n" {
		return Some(counter.to_string());
	}
	if let Some(pad) = token.strip_prefix("n:") {
		let width: usize = pad.parse().ok()?;
		return Some(format!("{counter:0width$}"));
	}
	if let Some(format) = token.strip_prefix("date:") {
		return Some(target.modified.format(format).to_string());
	}
	if let Some(format) = token.strip_prefix("captured:") {
		return Some(
			target
				.captured
				.unwrap_or(target.modified)
				.format(format)
				.to_string(),
		);
	}
	None
}

/// The first letter of each word up, the rest down.
fn title_case(text: &str) -> String {
	let mut out = String::with_capacity(text.len());
	let mut at_word_start = true;
	for c in text.chars() {
		if c.is_whitespace() || c == '_' || c == '-' {
			out.push(c);
			at_word_start = true;
		} else if at_word_start {
			out.extend(c.to_uppercase());
			at_word_start = false;
		} else {
			out.extend(c.to_lowercase());
		}
	}
	out
}

#[cfg(test)]
mod tests {
	use super::*;
	use chrono::TimeZone;

	fn target(name: &str, index: u64) -> Target {
		Target {
			name: name.to_string(),
			parent: "Trip".to_string(),
			index,
			modified: Local.with_ymd_and_hms(2024, 5, 6, 7, 8, 9).unwrap(),
			captured: None,
		}
	}

	#[test]
	fn each_rule_shapes_the_name_in_order() {
		let rules = [
			RenameRule::Replace {
				find: "IMG_".into(),
				with: String::new(),
				regex: false,
				whole_name: false,
			},
			RenameRule::Case {
				stem: CaseRule::Lower,
				extension: ExtensionCase::Lower,
			},
			RenameRule::Affix {
				prefix: "{parent}-".into(),
				suffix: "-{date:%Y}".into(),
			},
		];
		assert_eq!(
			apply(&rules, &target("IMG_0041.JPG", 0)).unwrap(),
			"Trip-0041-2024.jpg"
		);
	}

	#[test]
	fn a_sequence_counts_the_targets_and_a_template_names_the_whole() {
		let sequence = [RenameRule::Sequence {
			pattern: "IMG_{n:04}".into(),
			start: 10,
			step: 5,
		}];
		assert_eq!(
			apply(&sequence, &target("a.jpg", 0)).unwrap(),
			"IMG_0010.jpg"
		);
		assert_eq!(
			apply(&sequence, &target("b.jpg", 2)).unwrap(),
			"IMG_0020.jpg"
		);

		let template = [RenameRule::Template {
			pattern: "{date:%Y-%m-%d} {name} ({n}){ext}".into(),
		}];
		assert_eq!(
			apply(&template, &target("clip.MOV", 3)).unwrap(),
			"2024-05-06 clip (4).MOV"
		);
		assert_eq!(
			apply(&template, &target("README", 0)).unwrap(),
			"2024-05-06 README (1)"
		);
	}

	#[test]
	fn a_regex_replace_captures_and_a_bad_one_is_refused() {
		let rules = [RenameRule::Replace {
			find: r"^(\d{4})-(\d{2})".into(),
			with: "$2.$1".into(),
			regex: true,
			whole_name: false,
		}];
		assert_eq!(
			apply(&rules, &target("2024-05 trip.txt", 0)).unwrap(),
			"05.2024 trip.txt"
		);
		let broken = [RenameRule::Replace {
			find: "(".into(),
			with: String::new(),
			regex: true,
			whole_name: false,
		}];
		assert!(check_rules(&broken).is_err());
		assert!(check_rules(&[RenameRule::Sequence {
			pattern: "{n}".into(),
			start: 1,
			step: 0
		}])
		.is_err());
	}

	#[test]
	fn title_case_and_an_unknown_token_are_left_readable() {
		assert_eq!(title_case("my HOLIDAY_photos-2"), "My Holiday_Photos-2");
		let rules = [RenameRule::Template {
			pattern: "{name}{what}".into(),
		}];
		assert_eq!(apply(&rules, &target("x.txt", 0)).unwrap(), "x{what}");
	}
}
