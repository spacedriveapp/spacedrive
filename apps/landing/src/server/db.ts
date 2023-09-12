import { connect } from '@planetscale/database';
import { mysqlTable, serial, timestamp, varchar } from 'drizzle-orm/mysql-core';
import { drizzle } from 'drizzle-orm/planetscale-serverless';

import { env } from '~/env';

export { and, eq, or, type InferModel } from 'drizzle-orm';

const dbConnection = connect({
	url: env.DATABASE_URL
});

export const db = drizzle(dbConnection);

// Spacedrive Schema

export const waitlistTable = mysqlTable('waitlist', {
	id: serial('id').primaryKey(),
	cuid: varchar('cuid', {
		length: 26
	}).notNull(),
	email: varchar('email', {
		length: 255
	}).notNull(),
	created_at: timestamp('created_at').notNull()
});
