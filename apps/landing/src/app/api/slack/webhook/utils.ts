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
