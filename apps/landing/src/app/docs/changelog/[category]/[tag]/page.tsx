import { bundleMDX } from 'mdx-bundler';
import { getMDXComponent } from 'next-contentlayer2/hooks';
import { notFound } from 'next/navigation';
import { getRelease, githubFetch } from '~/app/api/github';
import { DocMDXComponents } from '~/components/mdx';
import { toTitleCase } from '~/utils/misc';

import { Markdown } from '../../../Markdown';
import { getReleasesCategories } from '../../data';

interface Props {
	params: { category: string; tag: string };
}

export async function generateStaticParams(): Promise<Array<Props['params']>> {
  try {
    const categories = await getReleasesCategories();

    // Handle null/undefined case
    if (!categories) return [];

    return categories.flatMap((c) =>
      c.docs.map((d) => ({
        category: c.slug,
        tag: d.slug
      }))
    );
  } catch (error) {
    // Return empty array if error occurs
    return [];
  }
}

export async function generateMetadata({ params }: Props) {
	const title = `${params.tag} - Spacedrive Documentation`;

	return {
		title,
		openGraph: { title },
		authors: { name: 'Spacedrive Technology Inc.' }
	};
}

export default async function Page({ params }: Props) {
	const release = await githubFetch(getRelease(params.tag));
	if (release.draft) notFound();

	const { code } = await bundleMDX({ source: processComments(release.body ?? '') });
	const MDXContent = getMDXComponent(code);

	return (
		<Markdown classNames="sm:mt-[105px] mt-6 min-h-screen px-8">
			<h5 className="mb-2 text-sm font-semibold text-primary lg:min-w-[700px]">
				{toTitleCase(params.category)}
			</h5>
			<h1>{params.tag}</h1>
			<MDXContent components={DocMDXComponents} />
		</Markdown>
	);
}

function processComments(body: string): string {
	const bodyLines = body.split('\n').map((l) => l.trim());

	// eslint-disable-next-line no-constant-condition
	while (true) {
		const commentStartIndex = bodyLines.findIndex((l) => l.startsWith('<!-- '));

		if (commentStartIndex === -1) break;
		const commentEndIndex = bodyLines.findIndex((l) => l.startsWith('-->'));

		if (bodyLines[commentStartIndex] === '<!-- web') {
			bodyLines.splice(commentStartIndex, 1);
			bodyLines.splice(commentEndIndex - 1, 1);
		} else bodyLines.splice(commentStartIndex, commentEndIndex - commentStartIndex + 1);
	}

	return bodyLines.join('\n');
}
