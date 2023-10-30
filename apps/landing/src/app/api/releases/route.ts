import { getRecentReleases, getReleaseFrontmatter, githubFetch } from '../github';

export const runtime = 'edge';

export async function GET() {
	const releases = await githubFetch(getRecentReleases);

	return Response.json(
		releases.map((release) => {
			return {
				...getReleaseFrontmatter(release),
				version: release.tag_name,
				published_at: release.published_at
			};
		})
	);
}
