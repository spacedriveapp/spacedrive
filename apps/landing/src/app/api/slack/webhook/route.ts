import { z } from 'zod';
import { env } from '~/env';

export const runtime = 'edge';

async function isValidSlackRequest(
	headers: Headers,
	body: string
): Promise<{ valid: true } | { valid: false; error: string }> {
	const signature = headers.get('x-slack-signature');
	if (!signature) return { valid: false, error: 'No signature' };

	const timestamp = headers.get('x-slack-request-timestamp');
	if (!timestamp) return { valid: false, error: 'No timestamp' };

	// todo: prevent replay attack

	const key = await crypto.subtle.importKey(
		'raw',
		new TextEncoder().encode(env.SLACK_SIGNING_SECRET),
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

	if (!valid) return { valid: false, error: 'Invalid signature' };

	return { valid: true };
}

const BODY = z.union([
	// https://api.slack.com/interactivity/slash-commands#app_command_handling
	z.discriminatedUnion('command', [
		z.object({
			command: z.literal('/release'),
			channel_id: z.string(),
			text: z.string().transform((s) => s.split(' ')),
			user_id: z.string()
		})
	]),
	// https://api.slack.com/reference/interaction-payloads/block-actions
	z.object({
		payload: z
			.string()
			.transform((v) => JSON.parse(v))
			.pipe(
				z.object({
					type: z.literal('block_actions'),
					actions: z.tuple([
						z.object({
							action_id: z.literal('createRelease'),
							value: z
								.string()
								.transform((v) => JSON.parse(v))
								.pipe(z.object({ tag: z.string(), commit: z.string() }))
						})
					]),
					user: z.object({
						id: z.string()
					}),
					response_url: z.string()
				})
			)
	})
]);

const GITHUB_API = `https://api.github.com`;
const GITHUB_REPO_API = `${GITHUB_API}/repos/${env.GITHUB_ORG}/${env.GITHUB_REPO}`;
const GITHUB_HEADERS = {
	'Authorization': `Bearer ${env.GITHUB_PAT}`,
	'Accept': 'application/vnd.github+json',
	'Content-Type': 'application/json'
};

export async function POST(req: Request) {
	const body = await req.text();

	const isValid = await isValidSlackRequest(req.headers, body);
	if (!isValid.valid) return new Response(isValid.error, { status: 400 });

	const parsedBody = BODY.safeParse(Object.fromEntries([...new URLSearchParams(body)]));
	if (!parsedBody.success) {
		console.log(parsedBody.error);
		return new Response('Unexpected request', { status: 400 });
	}

	if ('command' in parsedBody.data) {
		const { command, text, channel_id, user_id } = parsedBody.data;
		switch (command) {
			case '/release': {
				if (channel_id !== env.SLACK_RELEASES_CHANNEL) {
					return Response.json({
						response_type: 'ephemeral',
						text: `\`${command}\` can only be used in <#${env.SLACK_RELEASES_CHANNEL}>`
					});
				}

				const [tag, commitSha] = text;

				const existingBranch = await fetch(`${GITHUB_REPO_API}/branches/${tag}`, {
					headers: GITHUB_HEADERS
				});

				if (existingBranch.status !== 404)
					return Response.json({
						response_type: 'ephemeral',
						blocks: [
							{
								type: 'section',
								text: {
									type: 'mrkdwn',
									text: `<@${user_id}> A branch with the name \`${tag}\` already exists.`
								}
							}
						]
					});

				const commitData = await fetch(
					`${GITHUB_REPO_API}/commits/${commitSha ?? 'heads/main'}`,
					{ headers: GITHUB_HEADERS }
				).then((r) => r.json());

				return Response.json({
					response_type: 'in_channel',
					blocks: [
						{
							type: 'section',
							text: {
								type: 'mrkdwn',
								text: [
									`<@${user_id}> Are you sure you want to create this release?`,
									`*Make sure you've bumped the versions of sd-core and sd-desktop*`,
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
										text: 'View Commit'
									},
									url: `https://github.com/${env.GITHUB_ORG}/${env.GITHUB_REPO}/commit/${commitData.sha}`
								},
								{
									type: 'button',
									text: {
										type: 'plain_text',
										text: 'Create Release'
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
	} else if (parsedBody.data.payload) {
		const { payload } = parsedBody.data;

		switch (payload.type) {
			case 'block_actions': {
				const action = payload.actions[0];

				switch (action.action_id) {
					case 'createRelease': {
						const { value } = action;

						await fetch(`${GITHUB_REPO_API}/git/refs`, {
							method: 'POST',
							body: JSON.stringify({
								ref: `refs/tags/${value.tag}`,
								sha: value.commit
							}),
							headers: GITHUB_HEADERS
						}).then((r) => r.json());

						const createRelease = fetch(`${GITHUB_REPO_API}/releases`, {
							method: 'POST',
							body: JSON.stringify({
								tag_name: value.tag,
								name: value.tag,
								target_commitish: value.commit,
								draft: true,
								generate_release_notes: true
							}),
							headers: GITHUB_HEADERS
						}).then((r) => r.json());

						const dispatchWorkflowRun = fetch(
							`${GITHUB_REPO_API}/actions/workflows/release.yml/dispatches`,
							{
								method: 'POST',
								body: JSON.stringify({
									ref: value.tag
								}),
								headers: GITHUB_HEADERS
							}
						);
						const [release] = await Promise.all([createRelease, dispatchWorkflowRun]);

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
												`Go give it some release notes`,
												`*Created By* <@${payload.user.id}>`
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
			}
		}
	}
}
