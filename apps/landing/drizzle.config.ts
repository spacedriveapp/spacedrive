import 'dotenv/config';
import { Config } from 'drizzle-kit';
import { env } from './src/env';

export default {
	schema: ['./src/server/db.ts'],
	connectionString: env.DATABASE_URL
} satisfies Config;
