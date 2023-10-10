import 'dotenv/config';

import { Config } from 'drizzle-kit';

// TODO: Using t3 env is too damn hard, thanks JS bs
if (!process.env.DATABASE_URL) {
	throw new Error('DATABASE_URL is not set');
}

export default {
	schema: ['./src/server/db.ts'],
	connectionString: process.env.DATABASE_URL
} satisfies Config;
