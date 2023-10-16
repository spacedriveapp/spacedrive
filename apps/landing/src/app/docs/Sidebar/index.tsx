import { getRecentReleases, getReleaseFrontmatter, githubFetch } from '~/app/api/github';
import { iconConfig } from '~/utils/contentlayer';
import { toTitleCase } from '~/utils/util';

import { navigationMeta, SectionMeta } from '../data';
import { Categories, Doc } from './Categories';
import { SearchBar } from './Search';
import { SectionLink } from './SectionLink';

export async function Sidebar() {
	const releases = await githubFetch(getRecentReleases);

	const navigationWithReleases = [
		...navigationMeta,
		{
			slug: 'changelog',
			categories: (() => {
				const categories: Record<string, SectionMeta['categories'][number]> = {};

				for (const release of releases) {
					const { frontmatter } = getReleaseFrontmatter(release);

					const slug = frontmatter.category;

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
			})()
		}
	];

	const categoriesPerSection = navigationWithReleases.map((section) => ({
		slug: section.slug,
		categories: (
			<>
				{section.categories.map((category) => (
					<div className="mb-5" key={category.title}>
						<h2 className="font-semibold no-underline">{category.title}</h2>
						<ul className="mt-3">
							{category.docs.map((doc) => (
								<Doc
									key={doc.slug}
									slug={doc.slug}
									title={doc.title}
									url={doc.url}
								/>
							))}
						</ul>
					</div>
				))}
			</>
		)
	}));

	return (
		<nav className="mr-8 flex w-full flex-col sm:w-52">
			<SearchBar />
			<div className="mb-6 flex flex-col">
				{navigationWithReleases.map((section) => {
					const Icon = iconConfig[section.slug];

					return (
						<SectionLink
							// Use the first page in the section as the link
							href={section.categories[0].docs[0].url}
							key={section.slug}
							slug={section.slug}
						>
							<div className="mr-4 rounded-lg border-t border-gray-400/20 bg-gray-500 p-1">
								<Icon weight="bold" className="h-4 w-4 text-white opacity-80" />
							</div>
							{toTitleCase(section.slug)}
						</SectionLink>
					);
				})}
			</div>
			<Categories sections={categoriesPerSection} />
		</nav>
	);
}
