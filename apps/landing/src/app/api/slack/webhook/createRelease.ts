import { object, z } from 'zod';
import { env } from '~/env';

import * as github from './github';
import { createBlockActions, createSlashCommand, createViewSubmission, USER_REF } from './utils';

export const callbackId = 'createReleaseModal' as const;
export const fields = {
	category: {
		blockId: 'category',
		actionId: 'value'
	},
	tagline: {
		blockId: 'tagline',
		actionId: 'value'
	}
} as const;

export const COMMAND_NAME = '/release' as const;
export const EVENT_SCHEMAS = [createSlashCommand(COMMAND_NAME), createViewSubmission(), createBlockActions()] as const;

export async function createModal(
	trigger_id: string,
	tag: string,
	commit: string,
	commitMessage: string,
	responseUrl: string
) {
	return await fetch(`https://slack.com/api/views.open`, {
		method: 'POST',
		body: JSON.stringify({
			trigger_id,
			view: {
				type: 'modal',
				callback_id: callbackId,
				private_metadata: JSON.stringify({
					tag,
					commit,
					responseUrl
				}),
				title: {
					type: 'plain_text',
					text: `Release ${tag}`
				},
				submit: {
					type: 'plain_text',
					text: 'Create'
				},
				blocks: [
					{
						type: 'header',
						text: {
							type: 'plain_text',
							text: 'Commit'
						}
					},

					{
						type: 'section',
						accessory: {
							type: 'button',
							text: {
								type: 'plain_text',
								text: 'View Commit'
							},
							url: `${github.REPO_API}/commits/${commit}`
						},
						text: {
							type: 'mrkdwn',
							text: commitMessage
								.split('\n')
								.map((line) => `> ${line}`)
								.join('\n')
						}
					},
					{
						type: 'context',
						elements: [
							{
								type: 'mrkdwn',
								text: `${commit}`
							}
						]
					},
					{
						type: 'header',
						text: {
							type: 'plain_text',
							text: 'Make Sure You Have'
						}
					},
					{
						type: 'section',
						text: {
							type: 'mrkdwn',
							text: ['- Bumped versions of `sd-core` and `sd-desktop`'].join('\n')
						}
					},
					{
						type: 'header',
						text: {
							type: 'plain_text',
							text: 'Frontmatter'
						}
					},
					{
						type: 'context',
						elements: [
							{
								type: 'mrkdwn',
								text: `These values can be edited later in the release`
							}
						]
					},
					{
						type: 'input',
						block_id: fields.category.blockId,
						element: {
							type: 'static_select',
							action_id: fields.category.actionId,
							placeholder: {
								type: 'plain_text',
								text: 'Category'
							},
							options: [
								{
									text: {
										type: 'plain_text',
										text: 'Alpha'
									},
									value: 'alpha'
								}
							]
						},
						label: {
							type: 'plain_text',
							text: 'Category'
						}
					},
					{
						type: 'input',
						block_id: fields.tagline.blockId,
						element: {
							type: 'plain_text_input',
							action_id: fields.tagline.actionId,
							placeholder: {
								type: 'plain_text',
								text: 'Features A, B, and C await you!'
							}
						},
						label: {
							type: 'plain_text',
							text: 'Tagline'
						}
					},
					{
						type: 'context',
						elements: [
							{
								type: 'plain_text',
								text: `Show in the 'Update Available' toast`
							}
						]
					}
				]
			}
		}),
		headers: {
			'Authorization': `Bearer ${env.SLACK_BOT_TOKEN}`,
			'Content-Type': 'application/json'
		}
	});
}

export async function handleSubmission(
	values: Record<string, Record<string, any>>,
	user: z.infer<typeof USER_REF>,
	privateMetadata: string
) {
	console.log(values);

	const category =
		values[fields.category.blockId][fields.category.actionId].selected_option.value;
	const tagline = values[fields.tagline.blockId][fields.tagline.actionId].value;

	const { tag, commit, responseUrl } = JSON.parse(privateMetadata);

	const createTag = await fetch(`${github.REPO_API}/git/tags`, {
		method: 'POST',
		body: JSON.stringify({
			tag,
			message: tagline,
			object: commit,
			type: 'commit'
		}),
		headers: github.HEADERS
	}).then((r) => r.json());

	const getRef = await fetch(`${github.REPO_API}/git/refs`, {
		method: 'POST',
		body: JSON.stringify({
			ref: `refs/tags/${tag}`,
			sha: commit
		}),
		headers: github.HEADERS
	}).then((r) => r.json());

	const createRelease = fetch(`${github.REPO_API}/releases`, {
		method: 'POST',
		body: JSON.stringify({
			tag_name: tag,
			name: tag,
			target_commitish: commit,
			draft: true,
			generate_release_notes: true,
			body: [
				'<!-- frontmatter',
				'---',
				`category: ${category}`,
				`tagline: ${tagline}`,
				'---',
				'-->'
			].join('\n')
		}),
		headers: github.HEADERS
	}).then((r) => r.json());

	const dispatchWorkflowRun = fetch(
		`${github.REPO_API}/actions/workflows/release.yml/dispatches`,
		{
			method: 'POST',
			body: JSON.stringify({ ref: tag }),
			headers: github.HEADERS
		}
	);

	const [release] = await Promise.all([createRelease, dispatchWorkflowRun]);

	await fetch(responseUrl, {
		method: 'POST',
		body: JSON.stringify({
			replace_original: 'true',
			response_type: 'in_channel',
			blocks: [
				{
					type: 'section',
					block_id: '0',
					text: {
						type: 'mrkdwn',
						text: [
							`*Release \`${tag}\` created!*`,
							`Go give it some release notes`,
							`*Created By* <@${user.id}>`
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
							url: `https://github.com/${env.GITHUB_ORG}/${env.GITHUB_REPO}/commits/${commit}`
						}
					]
				}
			]
		}),
		headers: {
			'Content-Type': 'application/json'
		}
	});
}
