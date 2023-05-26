import { ProviderType } from '@auth/core/providers';
import { connect } from '@planetscale/database';
import { int, mysqlTable, primaryKey, serial, timestamp, varchar } from 'drizzle-orm/mysql-core';
import { drizzle } from 'drizzle-orm/planetscale-serverless';
import { env } from '~/env';

export { eq, and, or, type InferModel } from 'drizzle-orm';

const dbConnection = connect({
	url: env.DATABASE_URL
});

export const db = drizzle(dbConnection);

// AuthJS Schema

// Planetscale moment
const text = (name: string) =>
	varchar(name, {
		length: 255
	});

export const usersTable = mysqlTable('users', {
	id: text('id').notNull().primaryKey(),
	name: text('name'),
	email: text('email').notNull(),
	emailVerified: timestamp('emailVerified', { mode: 'date' }),
	image: text('image')
});

export const accountsTable = mysqlTable(
	'accounts',
	{
		userId: text('userId').notNull(),
		//   .references(() => users.id, { onDelete: "cascade" }),
		type: text('type').$type<ProviderType>().notNull(),
		provider: text('provider').notNull(),
		providerAccountId: text('providerAccountId').notNull(),
		refresh_token: text('refresh_token'),
		access_token: text('access_token'),
		expires_at: int('expires_at'),
		token_type: text('token_type'),
		scope: text('scope'),
		id_token: text('id_token'),
		session_state: text('session_state')
	},
	(account) => ({
		compoundKey: primaryKey(account.provider, account.providerAccountId)
	})
);

export const sessionsTable = mysqlTable('sessions', {
	sessionToken: text('sessionToken').notNull().primaryKey(),
	userId: text('userId').notNull(),
	// .references(() => users.id, { onDelete: "cascade" }),
	expires: timestamp('expires', { mode: 'date' }).notNull()
});

export const verificationTokens = mysqlTable(
	'verificationToken',
	{
		identifier: text('identifier').notNull(),
		token: text('token').notNull(),
		expires: timestamp('expires', { mode: 'date' }).notNull()
	},
	(vt) => ({
		compoundKey: primaryKey(vt.identifier, vt.token)
	})
);

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
