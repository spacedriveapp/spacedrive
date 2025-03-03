import { z } from 'zod';
import { env } from '~/env';

import { isValidSlackRequest } from './auth';
import * as createRelease from './createRelease';
import * as github from './github';

export const runtime = 'edge';

const BODY = z.union([...createRelease.EVENT_SCHEMAS]);

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
		const { command, text, channel_id, user_id, trigger_id, response_url } = parsedBody.data;
		switch (command) {
			case createRelease.COMMAND_NAME: {
				if (channel_id !== env.SLACK_RELEASES_CHANNEL) {
					return Response.json({
						response_type: 'ephemeral',
						text: `\`${command}\` can only be used in <#${env.SLACK_RELEASES_CHANNEL}>`
					});
				}

				const [tag, commitSha] = text;

				const existingBranch = await fetch(`${github.REPO_API}/branches/${tag}`, {
					headers: github.HEADERS
				});

				if (existingBranch.status === 200)
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
					`${github.REPO_API}/commits/${commitSha ?? 'heads/main'}`,
					{ headers: github.HEADERS }
				).then((r) => r.json());

				await createRelease.createModal(
					trigger_id,
					tag,
					commitData.sha,
					commitData.commit.message,
					response_url
				);

				break;
			}
		}
	} else if (parsedBody.data.payload) {
		const { payload } = parsedBody.data;

		switch (payload.type) {
			case 'view_submission': {
				switch (payload.view.callback_id) {
					case createRelease.callbackId: {
						await createRelease.handleSubmission(
							payload.view.state.values,
							payload.user,
							payload.view.private_metadata!
						);
					}
				}
			}
		}
	}

	return new Response('');
}
