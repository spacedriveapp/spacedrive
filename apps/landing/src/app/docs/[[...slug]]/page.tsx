import { allDocs } from '@contentlayer/generated';
import { CaretRight } from '@phosphor-icons/react/dist/ssr';
import { Github } from '@sd/assets/svgs/brands';
import { Metadata } from 'next';
import { getMDXComponent } from 'next-contentlayer/hooks';
import Link from 'next/link';
import { notFound } from 'next/navigation';
import { DocMDXComponents } from '~/components/mdx';
import { toTitleCase } from '~/utils/util';

import { getDoc } from './data';
import { Markdown } from './Markdown';

export function generateStaticParams() {
	const slugs = allDocs.map((doc) => doc.slug);
	return slugs.map((slug) => ({ slug: slug.split('/') }));
}

interface Props {
	params: { slug?: string[] };
}

export function generateMetadata({ params }: Props): Metadata {
	if (!params.slug)
		return {
			title: 'Spacedrive Docs',
			description: 'Learn more about Spacedrive'
		};

	return {};
}

export default function Page({ params }: Props) {
	if (!params.slug) return <Index />;

	const { doc, nextDoc } = getDoc(params.slug);

	if (!doc) notFound();

	const MDXContent = getMDXComponent(doc.body.code);

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

function BottomCard(props: any) {
	return (
		<div
			className="group flex flex-row items-center rounded-lg border border-gray-700 p-4 text-sm !text-gray-200 transition-all duration-200 hover:translate-y-[-2px] hover:border-primary hover:!text-primary hover:shadow-xl hover:shadow-primary/10"
			{...props}
		/>
	);
}

function Index() {
	return (
		<Markdown>
			<div className="mt-[105px]">
				<h1 className="text-4xl font-bold">Spacedrive Docs</h1>
				<p className="text-lg text-gray-400">
					Welcome to the Spacedrive documentation. Here you can find all the information
					you need to get started with Spacedrive.
				</p>
				<Link
					className="text-primary-600 transition hover:text-primary-500"
					href="/docs/product/getting-started/introduction"
				>
					Get Started →
				</Link>
			</div>
		</Markdown>
	);
}
