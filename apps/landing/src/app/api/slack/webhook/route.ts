import { Octokit } from 'octokit';
import { env } from '~/env';

export const runtime = 'edge';

export async function POST(req: Request) {
	const signature = req.headers.get('x-slack-signature');
	if (!signature) return new Response('No signature', { status: 400 });

	const timestamp = req.headers.get('x-slack-request-timestamp');
	if (!timestamp) return new Response('No timestamp', { status: 400 });

	const body = await req.text();

	// todo: prevent replay attack

	const key = await crypto.subtle.importKey(
		'raw',
		new TextEncoder().encode('7823b4c149599d16ecb77fd39f1b6b0f'),
		{ name: 'HMAC', hash: 'SHA-256' },
		false,
		['verify']
	);

	const valid = await crypto.subtle.verify(
		'HMAC',
		key,
		Buffer.from(signature.substring(3), 'hex'),
		new TextEncoder().encode(`v0:${timestamp}:${body}`)
	);

	if (!valid) return new Response('Invalid signature', { status: 400 });

	const parsedBody = Object.fromEntries([...new URLSearchParams(body)]);

	if (parsedBody.command) {
		switch (parsedBody.command) {
			case '/release': {
				const [tag, commitSha] = parsedBody.text.split(' ');

				const commitData = await fetch(
					`https://api.github.com/repos/${env.GITHUB_ORG}/${env.GITHUB_REPO}/commits/${
						commitSha ?? 'heads/main'
					}`,
					{
						headers: {
							Authorization: `Bearer ${env.GITHUB_PAT}`,
							Accept: 'application/vnd.github+json'
						}
					}
				).then((r) => r.json());

				return Response.json({
					response_type: 'in_channel',
					blocks: [
						{
							type: 'section',
							block_id: '0',
							text: {
								type: 'mrkdwn',
								text: [
									'Are you sure you want to create this release?',
									`*Version:* \`${tag}\``,
									`*Commit:* \`${commitData.sha}\``,
									`> ${commitData.commit.message.split('\n')[0]}`
								].join('\n')
							}
						},
						{
							type: 'actions',
							elements: [
								{
									type: 'button',
									text: {
										type: 'plain_text',
										text: 'Create'
									},
									value: JSON.stringify({ tag, commit: commitData.sha }),
									action_id: 'createRelease'
								}
							]
						}
					]
				});
			}
		}
	} else if (parsedBody.payload) {
		const payload = JSON.parse(parsedBody.payload);

		switch (payload.type) {
			case 'block_actions': {
				const action = payload.actions[0];

				switch (action.action_id) {
					case 'createRelease': {
						const value: { tag: string; commit: string } = JSON.parse(action.value);

						await fetch(
							`https://api.github.com/repos/${env.GITHUB_ORG}/${env.GITHUB_REPO}/git/refs`,
							{
								method: 'POST',
								body: JSON.stringify({
									ref: `refs/tags/${value.tag}`,
									sha: value.commit
								}),
								headers: {
									'Authorization': `Bearer ${env.GITHUB_PAT}`,
									'Accept': 'application/vnd.github+json',
									'Content-Type': 'application/json'
								}
							}
						).then((r) => r.json());

						const releaseFetch = fetch(
							`https://api.github.com/repos/${env.GITHUB_ORG}/${env.GITHUB_REPO}/releases`,
							{
								method: 'POST',
								body: JSON.stringify({
									tag_name: value.tag,
									name: value.tag,
									target_commitish: value.commit,
									draft: true
								}),
								headers: {
									'Authorization': `Bearer ${env.GITHUB_PAT}`,
									'Accept': 'application/vnd.github+json',
									'Content-Type': 'application/json'
								}
							}
						).then((r) => r.json());

						const workflowRunFetch = fetch(
							`https://api.github.com/repos/${env.GITHUB_ORG}/${env.GITHUB_REPO}/actions/workflows/release.yml/dispatches`,
							{
								method: 'POST',
								body: JSON.stringify({
									ref: value.tag
								}),
								headers: {
									'Authorization': `Bearer ${env.GITHUB_PAT}`,
									'Accept': 'application/vnd.github+json',
									'Content-Type': 'application/json'
								}
							}
						);

						const [release] = await Promise.all([releaseFetch, workflowRunFetch]);

						await fetch(payload.response_url, {
							method: 'POST',
							body: JSON.stringify({
								replace_original: 'true',
								blocks: [
									{
										type: 'section',
										block_id: '0',
										text: {
											type: 'mrkdwn',
											text: [
												`*Release \`${value.tag}\` created!*`,
												`Go give it some release notes.`,
												`*Created By:* <@${payload.user.id}>`
											].join('\n')
										}
									},
									{
										type: 'actions',
										elements: [
											{
												type: 'button',
												text: {
													type: 'plain_text',
													text: 'View Release'
												},
												url: release.html_url
											},
											{
												type: 'button',
												text: {
													type: 'plain_text',
													text: 'View Workflow Runs'
												},
												url: `https://github.com/${env.GITHUB_ORG}/${env.GITHUB_REPO}/actions/workflows/release.yml`
											},
											{
												type: 'button',
												text: {
													type: 'plain_text',
													text: 'View Commit'
												},
												url: `https://github.com/${env.GITHUB_ORG}/${env.GITHUB_REPO}/commit/${value.commit}`
											}
										]
									}
								]
							}),
							headers: {
								'Content-Type': 'application/json'
							}
						});

						return new Response();
					}
				}

				break;
			}
		}
	}

	return new Response('Unexpected request', { status: 400 });
}
