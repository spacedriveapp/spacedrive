import { Auth, AuthConfig } from '@auth/core';
import GitHubProvider from '@auth/core/providers/github';
import { Session } from '@auth/core/types';
// @ts-expect-error // TODO: No types cringe
import md5 from 'md5';
import { env } from '~/env';
import { accountsTable, db, sessionsTable, usersTable, verificationTokens } from '~/server/db';
import { DrizzleAdapterMySQL } from './authjs-adapter-drizzle-mysql';

export const runtime = 'edge';

export type TSession = {
	user: {
		id: string;
		name?: string;
		email: string;
		image: string;
	};
	expires: Session['expires'];
};

// function EmailProvider2(): EmailConfig {
// 	return {
// 		id: 'email',
// 		type: 'email',
// 		name: 'Email',
// 		server: { host: 'localhost', port: 25, auth: { user: '', pass: '' } },
// 		from: '',
// 		maxAge: 24 * 60 * 60,
// 		async sendVerificationRequest({ identifier: to, url: verificationLink }) {
// 			await sendEmail(
// 				to,
// 				'Sign in',
// 				{
// 					verification_link: verificationLink
// 				},
// 				loginEmailTemplate
// 			);
// 		}
// 	};
// }

function gravatarUrl(email: string) {
	return `https://www.gravatar.com/avatar/${md5(email.trim().toLowerCase())}?d=404&r=pg`;
}

export const authOptions: AuthConfig = {
	trustHost: true,
	secret: env.AUTH_SECRET,
	adapter: DrizzleAdapterMySQL(
		db as any,
		{
			users: usersTable,
			sessions: sessionsTable,
			verificationTokens,
			accounts: accountsTable
		} as any
	) as any,
	providers: [
		GitHubProvider({
			clientId: env.GITHUB_CLIENT_ID!,
			clientSecret: env.GITHUB_SECRET!
		}) as any
		// EmailProvider2()
	],
	callbacks: {
		session: async ({ session, user }) => {
			const s: TSession = {
				expires: session.expires,
				user: {
					id: user.id,
					name: user.name ?? undefined,
					email: user.email!,
					image: user.image ?? gravatarUrl(user.email!)
				}
			};
			return s;
		}
	}
	// events: {
	// 	async createUser({ user }) {
	// 		await sendEmail(
	// 			user.email!,
	// 			'Welcome to Fonedex!',
	// 			{
	// 				name: user.name
	// 			},
	// 			welcomeEmailHtml
	// 		);
	// 	}
	// }
};

const handler = (req: Request) => Auth(req, authOptions);

export const GET = handler;
export const POST = handler;
