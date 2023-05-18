import { Doc, allDocs } from '@contentlayer/generated';
import { Github } from '@icons-pack/react-simple-icons';
import { InferGetStaticPropsType } from 'next';
import { useMDXComponent } from 'next-contentlayer/hooks';
import { CaretRight } from 'phosphor-react';
import { PropsWithChildren } from 'react';
import { Helmet } from 'react-helmet';
import DocsLayout from '~/components/DocsLayout';
import Markdown from '~/components/Markdown';
import { DocMDXComponents } from '~/components/mdx';

export async function getStaticPaths() {
	const paths = allDocs.map((doc) => doc.url);
	return {
		paths,
		fallback: false
	};
}

export async function getStaticProps({ params }: { params: { slug: string } }) {
	const doc = allDocs.find((doc) => doc.slug === params.slug);

	if (!doc) {
		return {
			notFound: true
		};
	}

	const currentDocIndex = allDocs.findIndex((d) => d.slug === params.slug);
	const nextDoc: Doc | undefined = allDocs[currentDocIndex + 1];

	return {
		props: {
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

export default function DocPage({ doc, nextDoc }: InferGetStaticPropsType<typeof getStaticProps>) {
	const MDXContent = useMDXComponent(doc.body.code);

	return (
		<>
			<Helmet>
				<title>{doc?.title} - Spacedrive Documentation</title>
				{/* TODO: DOCS SEO */}
				{/* <meta name="description" content={description} />
				<meta property="og:title" content={post?.title} />
				<meta property="og:description" content={description} />
				<meta property="og:image" content={featured_image} />
				<meta content="summary_large_image" name="twitter:card" />
			<meta name="author" content={post?.primary_author?.name || 'Spacedrive Technology Inc.'} /> */}
			</Helmet>

			<DocsLayout doc={doc} navigation={navigation}>
				<Markdown classNames="sm:mt-[105px] mt-6 min-h-screen ">
					<h5 className="mb-2 text-sm font-semibold text-primary lg:min-w-[700px]">
						{doc.categoryName}
					</h5>
					<MDXContent components={DocMDXComponents} />
					<div className="mt-10 flex flex-col gap-3 sm:flex-row">
						<a
							target="_blank"
							rel="noreferrer"
							href={`https://github.com/spacedriveapp/spacedrive/blob/main/docs/${doc.url}.md`}
							className="w-full"
						>
							<BottomCard>
								<Github className="mr-3 w-5" />
								Edit this page on GitHub
							</BottomCard>
						</a>
						{nextDoc && (
							<a href={`/docs/${nextDoc.url}`} className="w-full">
								<BottomCard>
									<CaretRight className="mr-3 w-5" />
									Next article: {nextDoc?.title}
								</BottomCard>
							</a>
						)}
					</div>
				</Markdown>
			</DocsLayout>
		</>
	);
}
