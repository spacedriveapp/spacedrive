import { allDocs } from '@contentlayer/generated';
import { CaretRight } from '@phosphor-icons/react/dist/ssr';
import { Github } from '@sd/assets/svgs/brands';
import { useMDXComponent } from 'next-contentlayer/hooks';
import Link from 'next/link';
import { notFound } from 'next/navigation';
import { DocMDXComponents } from '~/components/mdx';
import { getDocsNavigation } from '~/utils/contentlayer';
import { toTitleCase } from '~/utils/util';

import { Markdown } from '../Markdown';

export function generateStaticParams() {
	const slugs = allDocs.map((doc) => doc.slug);
	return slugs.map((slug) => ({ slug: slug.split('/') }));
}

function BottomCard(props: any) {
	return (
		<div
			className="group flex flex-row items-center rounded-lg border border-gray-700 p-4 text-sm !text-gray-200 transition-all duration-200 hover:translate-y-[-2px] hover:border-primary hover:!text-primary hover:shadow-xl hover:shadow-primary/10"
			{...props}
		/>
	);
}

export default function Page({ params }: any) {
	const { doc, nextDoc } = getDoc(params.slug);

	if (!doc) notFound();

	const MDXContent = useMDXComponent(doc.body.code);

	return (
		<Markdown classNames="sm:mt-[105px] mt-6 min-h-screen ">
			<h5 className="mb-2 text-sm font-semibold text-primary lg:min-w-[700px]">
				{toTitleCase(doc.category)}
			</h5>
			<MDXContent components={DocMDXComponents} />
			<div className="mt-10 flex flex-col gap-3 sm:flex-row">
				<Link
					target="_blank"
					rel="noreferrer"
					href={`https://github.com/spacedriveapp/spacedrive/blob/main${doc.url}.mdx`}
					className="w-full"
				>
					<BottomCard>
						<Github className="mr-3 w-5" />
						Edit this page on GitHub
					</BottomCard>
				</Link>
				{nextDoc && (
					<Link href={nextDoc.url} className="w-full">
						<BottomCard>
							<CaretRight className="mr-3 w-5" />
							Next article: {nextDoc.title}
						</BottomCard>
					</Link>
				)}
			</div>
		</Markdown>
	);
}

function getDoc(params: string[]) {
	const slug = params.join('/');

	const doc = allDocs.find((doc) => doc.slug === slug);

	if (!doc) {
		return {
			notFound: true
		};
	}

	const docNavigation = getDocsNavigation(allDocs);

	// TODO: Doesn't work properly (can't skip categories)
	const docIndex = docNavigation
		.find((sec) => sec.slug == doc.section)
		?.categories.find((cat) => cat.slug == doc.category)
		?.docs.findIndex((d) => d.slug == doc.slug);

	const nextDoc =
		docNavigation
			.find((sec) => sec.slug == doc.section)
			?.categories.find((cat) => cat.slug == doc.category)?.docs[(docIndex || 0) + 1] || null;

	return {
		navigation: docNavigation,
		doc,
		nextDoc
	};
}
