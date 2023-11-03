import { getRecentReleases, getReleaseFrontmatter, githubFetch } from '~/app/api/github';
import { toTitleCase } from '~/utils/util';

import { SectionMeta } from '../data';

export async function getReleasesCategories(): Promise<SectionMeta['categories'][number][]> {
	const releases = await githubFetch(getRecentReleases);

	const categories: Record<string, SectionMeta['categories'][number]> = {};

	for (const release of releases) {
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
