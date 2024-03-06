import { Webhooks } from '@octokit/webhooks';
import { revalidatePath, revalidateTag } from 'next/cache';
import { headers } from 'next/headers';
import { env } from '~/env';

import { getLatestRelease, getRecentReleases, getRelease } from '..';

export const runtime = 'edge';

if (env.GITHUB_SECRET == null) console.warn('GITHUB_SECRET is not set, using dummy value');

const webhook = new Webhooks({
	secret: env.GITHUB_SECRET || '00000000000000000000000000000000000000000'
});

export async function POST(req: Request) {
	const hdrs = headers();

	await webhook.verifyAndReceive({
		id: hdrs.get('x-github-delivery')!,
		name: hdrs.get('x-github-event') as any,
		signature: hdrs.get('x-hub-signature')!,
		payload: await req.text()
	});

	return new Response();
}

webhook.on('release', ({ payload }) => {
	if (payload.release.draft) return;

	revalidateTag(getRelease(payload.release.tag_name).path);
	revalidateTag(getRecentReleases.path);
	revalidateTag(getLatestRelease.path);

	revalidatePath('/docs', 'layout');
	revalidatePath(`/docs/alpha/${payload.release.tag_name}`, 'page');
	revalidatePath(`/`, 'page');
});
