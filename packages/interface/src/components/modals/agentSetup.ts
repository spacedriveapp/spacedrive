/**
 * Setup an agent pastes to connect with a grant. The token is shown once, at
 * creation, so every snippet is built from it here and never refetched.
 * Files an agent may commit read the token from SPACEDRIVE_GRANT_TOKEN.
 */

export const TOKEN_ENV = 'SPACEDRIVE_GRANT_TOKEN';

export interface AgentSetupSnippet {
	id: string;
	label: string;
	/** Where the text goes, in one line. */
	hint: string;
	text: string;
}

export function agentSetupSnippets(
	url: string,
	token: string
): AgentSetupSnippet[] {
	const exportLine = `export ${TOKEN_ENV}='${token}'`;
	const mcpJson = JSON.stringify(
		{
			mcpServers: {
				spacedrive: {
					type: 'http',
					url,
					headers: {Authorization: `Bearer \${${TOKEN_ENV}}`}
				}
			}
		},
		null,
		2
	);
	const stdio = JSON.stringify(
		{
			mcpServers: {
				spacedrive: {
					command: 'npx',
					args: [
						'-y',
						'mcp-remote',
						url,
						'--header',
						'Authorization:${SPACEDRIVE_AUTH}'
					],
					env: {SPACEDRIVE_AUTH: `Bearer ${token}`}
				}
			}
		},
		null,
		2
	);
	return [
		{
			id: 'claude',
			label: 'Claude Code',
			hint: 'Run in the terminal where the agent works.',
			text: `claude mcp add --transport http spacedrive ${url} --header 'Authorization: Bearer ${token}'`
		},
		{
			id: 'codex',
			label: 'Codex',
			hint: 'Add to ~/.codex/config.toml, and export the token where Codex runs.',
			text: `[mcp_servers.spacedrive]\nurl = "${url}"\nbearer_token_env_var = "${TOKEN_ENV}"\n\n# In the shell that starts Codex:\n# ${exportLine}`
		},
		{
			id: 'json',
			label: '.mcp.json',
			hint: 'A project file. It reads the token from the environment, so it is safe to commit.',
			text: `${mcpJson}\n\n# In the shell that starts the agent:\n# ${exportLine}`
		},
		{
			id: 'stdio',
			label: 'stdio',
			hint: 'For clients that only start local commands. mcp-remote bridges stdio to the endpoint.',
			text: stdio
		}
	];
}
