// @ts-check
//
// Has to be `.mjs` so it can be imported in `next.config.mjs`.
// Next.js are so cringe for not having support for Typescript config files.
//
// Using `.mjs` with Drizzle Kit is seemingly impossible without `.ts` so we resort to `.js`.
// Why does JS make this shit so hard, I just wanna import the file.
//
import { createEnv } from '@t3-oss/env-nextjs';
import { z } from 'zod';

export const env = createEnv({
	server: {
		DATABASE_URL: z.string().url(),
		AWS_SES_ACCESS_KEY: z.string(),
		AWS_SES_SECRET_KEY: z.string(),
		AWS_SES_REGION: z.string(),
		MAILER_FROM: z.string().default('Spacedrive <no-reply@spacedrive.com>')
	},
	client: {},
	runtimeEnv: {
		DATABASE_URL: process.env.DATABASE_URL,
		AWS_SES_ACCESS_KEY: process.env.AWS_SES_ACCESS_KEY,
		AWS_SES_SECRET_KEY: process.env.AWS_SES_SECRET_KEY,
		AWS_SES_REGION: process.env.AWS_SES_REGION,
		MAILER_FROM: process.env.MAILER_FROM
	},
	// In dev or in eslint disable checking.
	// Kinda sucks for in dev but you don't need the whole setup to change the docs.
	skipValidation: process.env.VERCEL !== '1'
});
