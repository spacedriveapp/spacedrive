import { NextRequest } from 'next/server';
import { z } from 'zod';
import { env } from '~/env';

export const runtime = 'edge';

const feedbackSchema = z.object({
	feedback: z.string({
		required_error: 'Feedback is required',
		invalid_type_error: 'Feedback must be a string'
	}),
	emoji: z.string({
		required_error: 'Emoji is required',
		invalid_type_error: 'Emoji must be a string'
	})
});

export async function POST(req: NextRequest) {
	const result = feedbackSchema.safeParse(await req.json());
	if (!result.success) {
		return new Response(
			JSON.stringify({
				message: result.error.toString()
			}),
			{
				status: 400
			}
		);
	}
	try {
		const { feedback, emoji } = result.data;
		const slackWebhook = env.SLACK_FEEDBACK_URL;
		const slackMessage = {
			blocks: [
				{
					type: 'section',
					text: {
						type: 'mrkdwn',
						text: `${feedback} ${emoji}`
					}
				}
			]
		};
		await fetch(slackWebhook, {
			method: 'POST',
			body: JSON.stringify(slackMessage)
		});
		return new Response(undefined, {
			status: 204
		});
	} catch (error) {
		console.log(error)
		return new Response(
			JSON.stringify({
				message: "Something went wrong. Please try again."
			}),
			{
				status: 500,
				headers: {
					'Content-Type': 'application/json'
				}
			}
		);
	}

}
