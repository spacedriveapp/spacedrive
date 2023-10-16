import { bundleMDX } from 'mdx-bundler';
import { getMDXComponent } from 'next-contentlayer/hooks';
import { getRelease, githubFetch } from '~/app/api/github';
import { DocMDXComponents } from '~/components/mdx';
import { toTitleCase } from '~/utils/util';

import { Markdown } from '../../../Markdown';
import { getReleasesCategories } from '../../data';

export async function generateStaticParams() {
	const categories = await getReleasesCategories();

	return categories.flatMap((c) => c.docs.map((d) => ({ category: c.slug, tag: d.slug })));
}

export default async function Page({ params }: { params: { category: string; tag: string } }) {
	const release = await githubFetch(getRelease(params.tag));

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
