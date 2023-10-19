import { getLatestRelease, getReleaseFrontmatter, githubFetch } from '../../github';

export const runtime = 'edge';

export async function GET() {
	const release = await githubFetch(getLatestRelease);

	return Response.json({
        ...getReleaseFrontmatter(release),
        version: release.tag_name,
        published_at: release.published_at}
    );
}
