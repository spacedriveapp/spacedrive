import React, { useEffect } from 'react';
import { Helmet } from 'react-helmet';

import '../../atom-one.css';
import DocsLayout from '../../components/DocsLayout';
import Markdown from '../../components/Markdown';
import { SingleDocResponse } from './api';

function Page({ data }: { data: SingleDocResponse }) {
	return (
		<>
			<Helmet>
				<title>{data?.doc?.title} - Spacedrive Documentation</title>
				{/* <meta name="description" content={description} />
				<meta property="og:title" content={post?.title} />
				<meta property="og:description" content={description} />
				<meta property="og:image" content={featured_image} />
				<meta content="summary_large_image" name="twitter:card" />
				<meta name="author" content={post?.primary_author?.name || 'Spacedrive Technology Inc.'} /> */}
			</Helmet>
			<DocsLayout doc={data.doc} docsList={data.docsList}>
				<Markdown>
					<div dangerouslySetInnerHTML={{ __html: data?.doc?.html as string }} />
				</Markdown>
			</DocsLayout>
		</>
	);
}

export { Page };
