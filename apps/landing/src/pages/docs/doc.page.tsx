import { ChevronRightIcon } from '@heroicons/react/24/solid';
import { Github } from '@icons-pack/react-simple-icons';
import React, { PropsWithChildren, useEffect } from 'react';
import { Helmet } from 'react-helmet';

import '../../atom-one.css';
import DocsLayout from '../../components/DocsLayout';
import Markdown from '../../components/Markdown';
import { SingleDocResponse } from './api';

function BottomCard(props: PropsWithChildren) {
	return (
		<div className="flex flex-row items-center p-4 text-sm border border-gray-700 rounded-lg group !text-gray-200 hover:!text-primary hover:shadow-xl hover:border-primary hover:shadow-primary/10 transition-all duration-200 hover:-translate-y-[2px]">
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
				<Markdown>
					<h5 className="mb-2 text-sm font-semibold text-primary">{doc.categoryName}</h5>
					<div dangerouslySetInnerHTML={{ __html: doc?.html as string }} />
					<div className="flex flex-row gap-3 mt-10">
						<a
							target="_blank"
							rel="noreferrer"
							href={`https://github.com/spacedriveapp/spacedrive/blob/main/docs/${doc.url}.md`}
							className="w-full"
						>
							<BottomCard>
								<Github className="w-5 mr-3" />
								Edit this page on GitHub
							</BottomCard>
						</a>
						{nextDoc && (
							<a href={`/docs/${nextDoc.url}`} className="w-full">
								<BottomCard>
									<ChevronRightIcon className="w-5 mr-3" />
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
