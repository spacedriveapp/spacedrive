import { z } from 'zod';

export const USER_REF = z.object({
	id: z.string(),
	name: z.string()
});

function createInteraction<T extends string, TInner extends z.ZodObject<any>>(
	type: T,
	inner: TInner
) {
	return z.object({
		payload: z
			.string()
			.transform((v) => JSON.parse(v))
			.pipe(
				z
					.object({
						type: z.literal(type),
						user: USER_REF
					})
					.merge(inner)
			)
	});
}

const VIEW_SUBMISSION_INNER = z.object({
	view: z.object({
		id: z.string(),
		type: z.literal('modal'),
		callback_id: z.string(),
		state: z.object({
			values: z.record(z.record(z.any()))
		}),
		private_metadata: z.string().optional()
	})
});

export function createViewSubmission() {
	return createInteraction('view_submission', VIEW_SUBMISSION_INNER);
}

export function createSlashCommand<T extends string>(command: T) {
	return z.object({
		token: z.string(),
		team_id: z.string(),
		team_domain: z.string(),
		channel_id: z.string(),
		channel_name: z.string(),
		user_id: z.string(),
		user_name: z.string(),
		command: z.literal(command),
		text: z.string().transform((s) => s.split(' ')),
		api_app_id: z.string(),
		is_enterprise_install: z.union([z.literal('false'), z.literal('true')]).transform((v) => v === 'true'),
		response_url: z.string(),
		trigger_id: z.string()
	});
}
