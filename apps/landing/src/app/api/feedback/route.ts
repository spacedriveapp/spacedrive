import { NextRequest } from 'next/server';
import { z } from 'zod';
import { env } from '~/env';

export const runtime = 'edge';

const feedbackSchema = z.object({
	feedback: z.string({
		required_error: 'Feedback is required',
		invalid_type_error: 'Feedback must be a string'
	}),
	emoji: z
		.string({
			required_error: 'Emoji is required',
			invalid_type_error: 'Emoji must be a string'
		})
		.emoji()
		.max(2)
		.optional()
});

const CORS_HEADERS = {
	'Access-Control-Allow-Origin': '*',
	'Access-Control-Allow-Methods': 'POST, OPTIONS',
	'Access-Control-Allow-Headers': 'Content-Type, Authorization'
};

export async function POST(req: NextRequest) {
	const result = feedbackSchema.safeParse(await req.json());

	if (!result.success)
		return new Response(
			JSON.stringify({
				message: result.error.toString()
			}),
			{
				status: 400,
				headers: {
					...CORS_HEADERS
				}
			}
		);

	try {
		const { emoji, feedback } = result.data;

		const slackMessage = {
			blocks: [
				{
					type: 'section',
					text: {
						type: 'mrkdwn',
						text: [feedback, emoji].filter(Boolean).join('\n\n')
					}
				}
			]
		};
		await fetch(env.SLACK_FEEDBACK_URL, {
			method: 'POST',
			body: JSON.stringify(slackMessage)
		});

		return new Response(undefined, {
			status: 204,
			headers: {
				...CORS_HEADERS
			}
		});
	} catch (error) {
		console.error(error);
		return new Response(
			JSON.stringify({
				message: 'Something went wrong. Please try again.'
			}),
			{
				status: 500,
				headers: {
					'Content-Type': 'application/json',
					...CORS_HEADERS
				}
			}
		);
	}
}

export async function OPTIONS() {
	return new Response('', {
		status: 200,
		headers: {
			'Access-Control-Allow-Origin': '*',
			'Access-Control-Allow-Methods': 'POST, OPTIONS',
			'Access-Control-Allow-Headers': 'Content-Type, Authorization'
		}
	});
}
