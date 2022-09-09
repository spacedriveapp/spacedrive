import React, { useEffect } from 'react';
import { Helmet } from 'react-helmet';

import '../../atom-one.css';
import DocsSidebar from '../../components/DocsSidebar';
import Markdown from '../../components/Markdown';
import { Doc, SidebarCategory } from './api';

function Page({ doc, sidebar }: { doc: Doc; sidebar: SidebarCategory[] }) {
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
			{/*  */}
			<div className="flex items-start w-full">
				<aside className="sticky mt-32 mb-20 top-32">
					<DocsSidebar activePath={doc.url} data={sidebar} />
				</aside>

				{/* <div className="w-52"></div> */}
				<div className="w-full ">
					<Markdown classNames="">
						<div
							dangerouslySetInnerHTML={{
								__html: doc.html?.replaceAll(
									'<a href=',
									`<a target="_blank" rel="noreferrer" href=`
								) as string
							}}
						/>
					</Markdown>
				</div>
			</div>
		</>
	);
}

export { Page };
