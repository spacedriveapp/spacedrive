import { createEnv } from '@t3-oss/env-core';
import { z } from 'zod';

export const env = createEnv({
	clientPrefix: 'VITE_',
	client: {
		VITE_LANDING_ORIGIN: z.string().default('https://www.spacedrive.com')
	},
	runtimeEnv: import.meta.env,
	skipValidation: false,
	emptyStringAsUndefined: true
});
