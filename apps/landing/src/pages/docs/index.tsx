import { allDocs } from '@contentlayer/generated';
import { InferGetStaticPropsType } from 'next';
import Head from 'next/head';
import Link from 'next/link';
import DocsLayout from '~/components/DocsLayout';
import Markdown from '~/components/Markdown';
import PageWrapper from '~/components/PageWrapper';
import { getDocsNavigation } from '~/utils/contentlayer';

export function getStaticProps() {
	return { props: { navigation: getDocsNavigation(allDocs) } };
}

export default function DocHomePage({
	navigation
}: InferGetStaticPropsType<typeof getStaticProps>) {
	return (
		<PageWrapper>
			<Head>
				<title>Spacedrive Docs</title>
				<meta name="description" content="Learn more about Spacedrive" />
			</Head>

			<DocsLayout navigation={navigation}>
				<Markdown>
					<div className="mt-[105px]">
						<h1 className="text-4xl font-bold">Spacedrive Docs</h1>
						<p className="text-lg text-gray-400">
							Welcome to the Spacedrive documentation. Here you can find all the
							information you need to get started with Spacedrive.
						</p>
						<Link
							className="text-primary-600 transition hover:text-primary-500"
							href="/docs/product/getting-started/introduction"
						>
							Get Started →
						</Link>
					</div>
				</Markdown>
			</DocsLayout>
		</PageWrapper>
	);
}
