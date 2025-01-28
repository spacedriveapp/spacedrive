import { getRecentReleases, getReleaseFrontmatter, githubFetch } from '~/app/api/github';
import { toTitleCase } from '~/utils/misc';

import { SectionMeta } from '../data';

export async function getReleasesCategories(): Promise<SectionMeta['categories'][number][]> {
	const releases = await githubFetch(getRecentReleases);

	const categories: Record<string, SectionMeta['categories'][number]> = {};

	// Ensure releases is an array before iteration
	const releasesArray = Array.isArray(releases) ? releases : [];

	for (const release of releasesArray) {
		if (release.draft) continue;

		const { frontmatter } = getReleaseFrontmatter(release);

		const slug = frontmatter.category;
		if (!slug) continue;

		const category = (categories[slug] ??= {
			title: toTitleCase(slug),
			slug,
			docs: []
		});

		category.docs.push({
			slug: release.tag_name,
			title: release.tag_name,
			url: `/docs/changelog/${slug}/${release.tag_name}`
		});
	}

	return Object.values(categories);
}

export async function getLatestRelease(): Promise<{ tag: string; category: string } | undefined> {
	const releases = await githubFetch(getRecentReleases);

	for (const release of releases ?? []) {
		if (!release.draft)
			return {
				tag: release.tag_name,
				category: getReleaseFrontmatter(release).frontmatter.category
			};
	}

	return undefined;
}
