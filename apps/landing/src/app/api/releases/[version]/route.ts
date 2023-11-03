import { notFound } from 'next/navigation';

import { getRelease, getReleaseFrontmatter, githubFetch } from '../../github';

export const runtime = 'edge';

export async function GET(_: Request, { params }: { params: { version: string } }) {
	const release = await githubFetch(getRelease(params.version));
	if (release.draft) notFound();

	return Response.json({
		...getReleaseFrontmatter(release),
		version: release.tag_name,
		published_at: release.published_at
	});
}
