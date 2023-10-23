import { env } from '~/env';

export async function isValidSlackRequest(
	headers: Headers,
	body: string
): Promise<{ valid: true } | { valid: false; error: string }> {
	const signature = headers.get('x-slack-signature');
	if (!signature) return { valid: false, error: 'No signature' };

	const timestamp = headers.get('x-slack-request-timestamp');
	if (!timestamp) return { valid: false, error: 'No timestamp' };

	// todo: prevent replay attack

	const key = await crypto.subtle.importKey(
		'raw',
		new TextEncoder().encode(env.SLACK_SIGNING_SECRET),
		{ name: 'HMAC', hash: 'SHA-256' },
		false,
		['verify']
	);

	const valid = await crypto.subtle.verify(
		'HMAC',
		key,
		Buffer.from(signature.substring(3), 'hex'),
		new TextEncoder().encode(`v0:${timestamp}:${body}`)
	);

	if (!valid) return { valid: false, error: 'Invalid signature' };

	return { valid: true };
}
