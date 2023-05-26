/**
 * <div style={{display: "flex", justifyContent: "space-between", alignItems: "center", padding: 16}}>
 *  <p style={{fontWeight: "normal"}}>Official <a href="https://github.com/drizzle-team/drizzle-orm">Drizzle ORM</a> adapter for Auth.js / NextAuth.js.</p>
 *  <a href="https://github.com/drizzle-team/drizzle-orm">
 *   <img style={{display: "block"}} src="https://pbs.twimg.com/profile_images/1598308842391179266/CtXrfLnk_400x400.jpg" width="38" />
 *  </a>
 * </div>
 *
 * ## Installation
 *
 * ```bash npm2yarn2pnpm
 * npm install next-auth drizzle-orm @next-auth/drizzle-adapter
 * npm install drizzle-kit --save-dev
 * ```
 *
 * @module @next-auth/drizzle-adapter
 */
import type { Adapter } from '@auth/core/adapters';
import { and, eq } from 'drizzle-orm';
// @ts-expect-error
import { v4 as uuid } from 'uuid';
import type { DbClient, Schema } from './schema';

/**
 * ## Setup
 *
 * Add this adapter to your `pages/api/[...nextauth].js` next-auth configuration object:
 *
 * ```js title="pages/api/auth/[...nextauth].js"
 * import NextAuth from "next-auth"
 * import GoogleProvider from "next-auth/providers/google"
 * import { DrizzleAdapter } from "@next-auth/drizzle-adapter"
 * import { db } from "./db-schema"
 *
 * export default NextAuth({
 *   adapter: DrizzleAdapter(db),
 *   providers: [
 *     GoogleProvider({
 *       clientId: process.env.GOOGLE_CLIENT_ID,
 *       clientSecret: process.env.GOOGLE_CLIENT_SECRET,
 *     }),
 *   ],
 * })
 * ```
 *
 * ## Advanced usage
 *
 * ### Create the Drizzle schema from scratch
 *
 * You'll need to create a database schema that includes the minimal schema for a `next-auth` adapter.
 * Be sure to use the Drizzle driver version that you're using for your project.
 *
 * > This schema is adapted for use in Drizzle and based upon our main [schema](https://authjs.dev/reference/adapters#models)
 *
 *
 * ```json title="db-schema.ts"
 *
 * import { integer, pgTable, text, primaryKey } from 'drizzle-orm/pg-core';
 * import { drizzle } from 'drizzle-orm/node-postgres';
 * import { migrate } from 'drizzle-orm/node-postgres/migrator';
 * import { Pool } from 'pg'
 * import { ProviderType } from 'next-auth/providers';
 *
 * export const users = pgTable('users', {
 * id: text('id').notNull().primaryKey(),
 * name: text('name'),
 * email: text("email").notNull(),
 * emailVerified: integer("emailVerified"),
 * image: text("image"),
 * });
 *
 * export const accounts = pgTable("accounts", {
 *  userId: text("userId").notNull().references(() => users.id, { onDelete: "cascade" }),
 *  type: text("type").$type<ProviderType>().notNull(),
 *  provider: text("provider").notNull(),
 *  providerAccountId: text("providerAccountId").notNull(),
 *  refresh_token: text("refresh_token"),
 *  access_token: text("access_token"),
 *  expires_at: integer("expires_at"),
 *  token_type: text("token_type"),
 *  scope: text("scope"),
 *  id_token: text("id_token"),
 *  session_state: text("session_state"),
 * }, (account) => ({
 *   _: primaryKey(account.provider, account.providerAccountId)
 * }))
 *
 * export const sessions = pgTable("sessions", {
 *  userId: text("userId").notNull().references(() => users.id, { onDelete: "cascade" }),
 *  sessionToken: text("sessionToken").notNull().primaryKey(),
 *  expires: integer("expires").notNull(),
 * })
 *
 * export const verificationTokens = pgTable("verificationToken", {
 *  identifier: text("identifier").notNull(),
 *  token: text("token").notNull(),
 *  expires: integer("expires").notNull()
 * }, (vt) => ({
 *   _: primaryKey(vt.identifier, vt.token)
 * }))
 *
 * const pool = new Pool({
 *   connectionString: "YOUR_CONNECTION_STRING"
 * });
 *
 * export const db = drizzle(pool);
 *
 * migrate(db, { migrationsFolder: "./drizzle" })
 *
 * ```
 *
 **/
export function DrizzleAdapterMySQL(
	client: DbClient,
	{ users, sessions, verificationTokens, accounts }: Schema
): Adapter {
	return {
		createUser: async (data) => {
			const id = uuid();

			await client.insert(users).values({ ...data, id });

			return client
				.select()
				.from(users)
				.where(eq(users.id, id))
				.then((res) => res[0]);
		},
		getUser: async (data) => {
			return (
				client
					.select()
					.from(users)
					.where(eq(users.id, data))
					.then((res) => res[0]) ?? null
			);
		},
		getUserByEmail: async (data) => {
			return (
				client
					.select()
					.from(users)
					.where(eq(users.email, data))
					.then((res) => res[0]) ?? null
			);
		},
		createSession: async (data) => {
			await client.insert(sessions).values(data);

			return client
				.select()
				.from(sessions)
				.where(eq(sessions.sessionToken, data.sessionToken))
				.then((res) => res[0]);
		},
		getSessionAndUser: async (data) => {
			return (
				client
					.select({
						session: sessions,
						user: users
					})
					.from(sessions)
					.where(eq(sessions.sessionToken, data))
					.innerJoin(users, eq(users.id, sessions.userId))
					.then((res) => res[0]) ?? null
			);
		},
		updateUser: async (data) => {
			if (!data.id) {
				throw new Error('No user id.');
			}

			await client.update(users).set(data).where(eq(users.id, data.id));

			return client
				.select()
				.from(users)
				.where(eq(users.id, data.id))
				.then((res) => res[0]);
		},
		updateSession: async (data) => {
			await client
				.update(sessions)
				.set(data)
				.where(eq(sessions.sessionToken, data.sessionToken));

			return client
				.select()
				.from(sessions)
				.where(eq(sessions.sessionToken, data.sessionToken))
				.then((res) => res[0]);
		},
		linkAccount: async (rawAccount) => {
			await client
				.insert(accounts)
				.values(rawAccount)
				.then((res) => res[0]);
		},
		getUserByAccount: async (account) => {
			const user =
				(await client
					.select()
					.from(users)
					.innerJoin(
						accounts,
						and(
							eq(accounts.providerAccountId, account.providerAccountId),
							eq(accounts.provider, account.provider)
						)
					)
					.then((res) => res[0])) ?? null;

			return user?.users;
		},
		deleteSession: async (sessionToken) => {
			await client.delete(sessions).where(eq(sessions.sessionToken, sessionToken));
		},
		createVerificationToken: async (token) => {
			await client.insert(verificationTokens).values(token);

			return client
				.select()
				.from(verificationTokens)
				.where(eq(verificationTokens.identifier, token.identifier))
				.then((res) => res[0]);
		},
		useVerificationToken: async (token) => {
			try {
				const deletedToken =
					(await client
						.select()
						.from(verificationTokens)
						.where(
							and(
								eq(verificationTokens.identifier, token.identifier),
								eq(verificationTokens.token, token.token)
							)
						)
						.then((res) => res[0])) ?? null;

				await client
					.delete(verificationTokens)
					.where(
						and(
							eq(verificationTokens.identifier, token.identifier),
							eq(verificationTokens.token, token.token)
						)
					);

				return deletedToken;
			} catch (err) {
				throw new Error('No verification token found.');
			}
		},
		deleteUser: async (id) => {
			await client
				.delete(users)
				.where(eq(users.id, id))
				.then((res) => res[0]);
		},
		unlinkAccount: async (account) => {
			await client
				.delete(accounts)
				.where(
					and(
						eq(accounts.providerAccountId, account.providerAccountId),
						eq(accounts.provider, account.provider)
					)
				);

			return undefined;
		}
	};
}
