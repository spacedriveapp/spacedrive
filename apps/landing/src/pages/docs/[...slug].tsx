import { allDocs } from '@contentlayer/generated';
import { SiGithub } from '@icons-pack/react-simple-icons';
import { InferGetStaticPropsType } from 'next';
import { useMDXComponent } from 'next-contentlayer/hooks';
import Head from 'next/head';
import Link from 'next/link';
import { CaretRight } from 'phosphor-react';
import { PropsWithChildren } from 'react';
import DocsLayout from '~/components/DocsLayout';
import Markdown from '~/components/Markdown';
import PageWrapper from '~/components/PageWrapper';
import { DocMDXComponents } from '~/components/mdx';
import { getDocsNavigation } from '~/utils/contentlayer';
import { toTitleCase } from '~/utils/util';

export async function getStaticPaths() {
	const paths = allDocs.map((doc) => doc.url);
	return {
		paths,
		fallback: false
	};
}

export async function getStaticProps({ params }: { params: { slug: string[] } }) {
	const slug = params.slug.join('/');

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
		props: {
			navigation: docNavigation,
			doc,
			nextDoc
		}
	};
}

function BottomCard(props: PropsWithChildren) {
	return (
		<div className="group flex flex-row items-center rounded-lg border border-gray-700 p-4 text-sm !text-gray-200 transition-all duration-200 hover:translate-y-[-2px] hover:border-primary hover:!text-primary hover:shadow-xl hover:shadow-primary/10">
			{props.children}
		</div>
	);
}

export default function DocPage({
	navigation,
	doc,
	nextDoc
}: InferGetStaticPropsType<typeof getStaticProps>) {
	const MDXContent = useMDXComponent(doc.body.code);

	return (
		<PageWrapper>
			<Head>
				<title>{doc.title} - Spacedrive Documentation</title>
				<meta name="description" content={doc.excerpt} />
				<meta property="og:title" content={doc.title} />
				<meta property="og:description" content={doc.excerpt} />
				{/* <meta property="og:image" content={featured_image} /> */}
				{/* <meta content="summary_large_image" name="twitter:card" /> */}
				{/* <meta name="author" content={post?.primary_author?.name || 'Spacedrive Technology Inc.'} /> */}
				<link
					rel="stylesheet"
					href="https://cdn.jsdelivr.net/npm/katex@0.16.0/dist/katex.min.css"
					integrity="sha384-Xi8rHCmBmhbuyyhbI88391ZKP2dmfnOl4rT9ZfRI7mLTdk1wblIUnrIq35nqwEvC"
					crossOrigin="anonymous"
				/>
			</Head>

			<DocsLayout docUrl={doc.url} navigation={navigation}>
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
								<SiGithub className="mr-3 w-5" />
								Edit this page on GitHub
							</BottomCard>
						</Link>
						{nextDoc && (
							<a href={nextDoc.url} className="w-full">
								<BottomCard>
									<CaretRight className="mr-3 w-5" />
									Next article: {nextDoc.title}
								</BottomCard>
							</a>
						)}
					</div>
				</Markdown>
			</DocsLayout>
		</PageWrapper>
	);
}
