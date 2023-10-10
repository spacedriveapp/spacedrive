import { allDocs } from '@contentlayer/generated';
import { CaretRight } from '@phosphor-icons/react';
import { Github } from '@sd/assets/svgs/brands';
import { InferGetStaticPropsType } from 'next';
import { useMDXComponent } from 'next-contentlayer/hooks';
import Head from 'next/head';
import Link from 'next/link';
import { PropsWithChildren } from 'react';
import DocsLayout from '~/components/DocsLayout';
import Markdown from '~/components/Markdown';
import { DocMDXComponents } from '~/components/mdx';
import PageWrapper from '~/components/PageWrapper';
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
				<title>{`${doc.title} - Spacedrive Documentation`}</title>
				<meta name="description" content={doc.description} />
				<meta property="og:title" content={doc.title} />
				<meta property="og:description" content={doc.description} />
				<meta name="author" content={'Spacedrive Technology Inc.'} />
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
			</DocsLayout>
		</PageWrapper>
	);
}
