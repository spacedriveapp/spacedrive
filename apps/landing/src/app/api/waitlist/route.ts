import type { NextRequest } from 'next/server';
import { z } from 'zod';
import { sendEmail } from '~/server/aws';
import { db, eq, waitlistTable } from '~/server/db';

import { welcomeTemplate } from './welcomeEmail';

export const runtime = 'edge';

const emailSchema = z.object({
	email: z
		.string({
			required_error: 'Email is required',
			invalid_type_error: 'Email must be a string'
		})
		.email({
			message: 'Invalid email address'
		})
		.transform((value) => value.toLowerCase())
});

function randomId(len = 10) {
	if (len % 2 !== 0) throw new Error('len must be a multiple of 2');
	const array = new Uint8Array(len / 2); // 1 char int to 2 chars hex
	self.crypto.getRandomValues(array);
	return [...array].map((c) => c.toString(16).padStart(2, '0')).join('');
}

export async function POST(req: NextRequest) {
	const result = emailSchema.safeParse(await req.json());
	if (!result.success) {
		return new Response(
			JSON.stringify({
				message: result.error.toString()
			}),
			{
				status: 400
			}
		);
	}

	const { email } = result.data;

	try {
		const emailExist = await db
			.select({ email: waitlistTable.email })
			.from(waitlistTable)
			.where(eq(waitlistTable.email, email));
		if (emailExist.length > 0) {
			return new Response(undefined, {
				status: 204
			});
		}

		const unsubId = randomId(26);
		await db.insert(waitlistTable).values({
			cuid: unsubId,
			email,
			created_at: new Date()
		});

		await sendEmail(
			email,
			'Welcome to Spacedrive',
			welcomeTemplate(`https://spacedrive.com/?wunsub=${unsubId}`)
		);

		return new Response(null, {
			status: 204
		});
	} catch (err) {
		console.error(err);
		return new Response(
			JSON.stringify({
				message: 'Something went wrong while trying to create invite'
			}),
			{
				status: 500,
				headers: {
					'Content-Type': 'application/json'
				}
			}
		);
	}
}

export async function DELETE(req: NextRequest) {
	const url = new URL(req.url);

	const id = url.searchParams.get('i');
	if (!id)
		return new Response(JSON.stringify(undefined), {
			status: 400
		});

	try {
		await db.delete(waitlistTable).where(eq(waitlistTable.cuid, id));

		return new Response(null, {
			status: 204
		});
	} catch (err) {
		console.error(err);
		return new Response(
			JSON.stringify({
				message: 'Something went wrong while trying to unsubscribe from waitlist'
			}),
			{
				status: 500,
				headers: {
					'Content-Type': 'application/json'
				}
			}
		);
	}
}
