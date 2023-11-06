import { getRecentReleases, getReleaseFrontmatter, githubFetch } from '../github';

export const runtime = 'edge';

export async function GET() {
	const releases = await githubFetch(getRecentReleases);

	return Response.json(
		releases
			.filter((r) => !r.draft)
			.map((release) => {
				return {
					...getReleaseFrontmatter(release),
					version: release.tag_name,
					published_at: release.published_at
				};
			})
	);
}

export async function OPTIONS() {
	return new Response('', {
		status: 200,
		headers: {
			'Access-Control-Allow-Origin': '*',
			'Access-Control-Allow-Methods': 'GET, OPTIONS'
		}
	});
}
