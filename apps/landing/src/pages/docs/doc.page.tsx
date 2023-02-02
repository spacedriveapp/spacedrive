import { Github } from '@icons-pack/react-simple-icons';
import { CaretRight } from 'phosphor-react';
import { PropsWithChildren } from 'react';
import { Helmet } from 'react-helmet';
import '../../atom-one.css';
import DocsLayout from '../../components/DocsLayout';
import Markdown from '../../components/Markdown';
import { SingleDocResponse, toTitleCase } from './api';

function BottomCard(props: PropsWithChildren) {
	return (
		<div className="hover:!text-primary hover:border-primary hover:shadow-primary/10 group flex flex-row items-center rounded-lg border border-gray-700 p-4 text-sm !text-gray-200 transition-all duration-200 hover:-translate-y-[2px] hover:shadow-xl">
			{props.children}
		</div>
	);
}

function Page({ doc, navigation, nextDoc }: SingleDocResponse) {
	if (!doc) return <>{JSON.stringify(doc)}</>;
	return (
		<>
			<Helmet>
				<title>{doc?.title} - Spacedrive Documentation</title>
				{/* <meta name="description" content={description} />
				<meta property="og:title" content={post?.title} />
				<meta property="og:description" content={description} />
				<meta property="og:image" content={featured_image} />
				<meta content="summary_large_image" name="twitter:card" />
			<meta name="author" content={post?.primary_author?.name || 'Spacedrive Technology Inc.'} /> */}
			</Helmet>

			<DocsLayout doc={doc} navigation={navigation}>
				<Markdown classNames="sm:mt-[105px] mt-6 min-h-screen ">
					<h5 className="text-primary mb-2 text-sm font-semibold lg:min-w-[700px]">
						{doc.categoryName}
					</h5>
					<div dangerouslySetInnerHTML={{ __html: doc?.html as string }} />
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

export { Page };
