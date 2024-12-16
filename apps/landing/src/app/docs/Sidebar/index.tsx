import { iconConfig } from '~/utils/contentlayer';
import { toTitleCase } from '~/utils/misc';

import { getReleasesCategories } from '../changelog/data';
import { navigationMeta } from '../data';
import { Categories, Doc } from './Categories';
import { SearchBar } from './Search';
import { SectionLink } from './SectionLink';

export async function Sidebar() {
	const navigationWithReleases = [
		...navigationMeta,
		{
			slug: 'changelog',
			categories: await getReleasesCategories()
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

					const href = section.categories[0]?.docs[0]?.url;

					if (!href) return null;

					return (
						<SectionLink
							// Use the first page in the section as the link
							href={href}
							key={section.slug}
							slug={section.slug}
						>
							<div className="mr-4 rounded-lg border-t border-gray-400/20 bg-gray-500 p-1">
								<Icon weight="bold" className="size-4 text-white opacity-80" />
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
