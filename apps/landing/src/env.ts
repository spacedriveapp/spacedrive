import { createEnv } from '@t3-oss/env-nextjs';
import { z } from 'zod';

export const env = createEnv({
	server: {
		DATABASE_URL: z.string().url(),
		SLACK_FEEDBACK_URL: z.string().url(),
		AUTH_SECRET: z.string(),
		GITHUB_PAT: z.string(),
		GITHUB_CLIENT_ID: z.string(),
		GITHUB_SECRET: z.string(),
		AWS_SES_ACCESS_KEY: z.string(),
		AWS_SES_SECRET_KEY: z.string(),
		AWS_SES_REGION: z.string(),
		MAILER_FROM: z.string().default('Spacedrive <no-reply@spacedrive.com>')
	},
	client: {},
	runtimeEnv: {
		DATABASE_URL: process.env.DATABASE_URL,
		SLACK_FEEDBACK_URL: process.env.SLACK_FEEDBACK_URL,
		AUTH_SECRET: process.env.AUTH_SECRET,
		GITHUB_PAT: process.env.GITHUB_PAT,
		GITHUB_CLIENT_ID: process.env.GITHUB_CLIENT_ID,
		GITHUB_SECRET: process.env.GITHUB_SECRET,
		AWS_SES_ACCESS_KEY: process.env.AWS_SES_ACCESS_KEY,
		AWS_SES_SECRET_KEY: process.env.AWS_SES_SECRET_KEY,
		AWS_SES_REGION: process.env.AWS_SES_REGION,
		MAILER_FROM: process.env.MAILER_FROM
	},
	// In dev or in eslint disable checking.
	// Kinda sucks for in dev but you don't need the whole setup to change the docs.
	skipValidation: process.env.VERCEL !== '1'
});
