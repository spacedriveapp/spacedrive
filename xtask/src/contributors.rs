use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

const REPO: &str = "spacedriveapp/spacedrive";
const OUTPUT_PATH: &str = "packages/interface/src/contributors.json";
const EXCLUDED_LOGINS: &[&str] = &["cursoragent"];

#[derive(Deserialize)]
struct GitHubContributor {
	login: String,
	#[serde(rename = "type")]
	account_type: String,
}

#[derive(Deserialize)]
struct GitHubUser {
	name: Option<String>,
}

#[derive(Serialize)]
struct Contributor {
	name: String,
	github: String,
}

/// Try to get a GitHub token from the environment or `gh` CLI
fn get_github_token() -> Option<String> {
	if let Ok(token) = std::env::var("GITHUB_TOKEN") {
		return Some(token);
	}

	std::process::Command::new("gh")
		.args(["auth", "token"])
		.output()
		.ok()
		.and_then(|o| {
			if o.status.success() {
				String::from_utf8(o.stdout)
					.ok()
					.map(|s| s.trim().to_string())
			} else {
				None
			}
		})
}

fn github_get(
	client: &reqwest::blocking::Client,
	url: &str,
	token: Option<&str>,
) -> reqwest::blocking::RequestBuilder {
	let mut req = client.get(url);
	if let Some(token) = token {
		req = req.bearer_auth(token);
	}
	req
}

pub fn update(project_root: &Path) -> Result<()> {
	println!("Fetching contributors from GitHub...");

	let token = get_github_token();
	if token.is_some() {
		println!("  using authenticated requests");
	} else {
		println!("  no token found, using unauthenticated requests (may hit rate limits)");
		println!("  tip: install `gh` CLI and run `gh auth login` for higher limits");
	}

	let client = reqwest::blocking::Client::builder()
		.user_agent("spacedrive-xtask")
		.timeout(std::time::Duration::from_secs(20))
		.build()
		.context("Failed to build HTTP client")?;

	// Paginate through all contributors
	let mut all_contributors = Vec::new();
	let mut page = 1u32;

	loop {
		let url = format!(
			"https://api.github.com/repos/{}/contributors?per_page=100&page={}",
			REPO, page
		);

		let resp: Vec<GitHubContributor> = github_get(&client, &url, token.as_deref())
			.send()
			.context("Failed to fetch contributors")?
			.json()
			.context("Failed to parse contributors response")?;

		if resp.is_empty() {
			break;
		}

		all_contributors.extend(resp);
		page += 1;
	}

	// Filter out bots and excluded accounts
	let humans: Vec<_> = all_contributors
		.iter()
		.filter(|c| c.account_type == "User" && !EXCLUDED_LOGINS.contains(&c.login.as_str()))
		.collect();

	println!("Found {} contributors, resolving names...", humans.len());

	let mut contributors = Vec::new();

	for (i, contributor) in humans.iter().enumerate() {
		let name = match resolve_name(&client, &contributor.login, token.as_deref()) {
			Ok(Some(n)) => n,
			_ => contributor.login.clone(),
		};

		contributors.push(Contributor {
			name,
			github: contributor.login.clone(),
		});

		// Progress indicator every 25 users
		if (i + 1) % 25 == 0 {
			println!("  resolved {}/{}", i + 1, humans.len());
		}
	}

	println!("  resolved {}/{}", contributors.len(), humans.len());

	let output_path = project_root.join(OUTPUT_PATH);
	let json =
		serde_json::to_string_pretty(&contributors).context("Failed to serialize contributors")?;

	std::fs::write(&output_path, format!("{}\n", json))
		.context("Failed to write contributors.json")?;

	println!(
		"Wrote {} contributors to {}",
		contributors.len(),
		OUTPUT_PATH
	);

	Ok(())
}

fn resolve_name(
	client: &reqwest::blocking::Client,
	login: &str,
	token: Option<&str>,
) -> Result<Option<String>> {
	let url = format!("https://api.github.com/users/{}", login);
	let user: GitHubUser = github_get(client, &url, token)
		.send()
		.context("Failed to fetch user")?
		.error_for_status()
		.context("GitHub API returned an error")?
		.json()
		.context("Failed to parse user response")?;

	Ok(user.name.filter(|n: &String| !n.is_empty()))
}
