import { ProviderType } from '@auth/core/providers';
import { drizzle } from 'drizzle-orm/mysql2';
import { int, mysqlTable, primaryKey, text, timestamp } from 'drizzle-orm/mysql-core';

// import mysql from "mysql2/promise";

export const users = mysqlTable('users', {
	id: text('id').notNull().primaryKey(),
	name: text('name'),
	email: text('email').notNull(),
	emailVerified: timestamp('emailVerified', { mode: 'date' }),
	image: text('image')
});

export const accounts = mysqlTable(
	'accounts',
	{
		userId: text('userId')
			.notNull()
			.references(() => users.id, { onDelete: 'cascade' }),
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

export const sessions = mysqlTable('sessions', {
	sessionToken: text('sessionToken').notNull().primaryKey(),
	userId: text('userId')
		.notNull()
		.references(() => users.id, { onDelete: 'cascade' }),
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

export const db = drizzle(undefined as any); // We just want the types

export type DbClient = typeof db;

export type Schema = {
	users: typeof users;
	accounts: typeof accounts;
	sessions: typeof sessions;
	verificationTokens: typeof verificationTokens;
};
