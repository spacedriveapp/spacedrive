import React, { useEffect } from 'react';
import { Helmet } from 'react-helmet';

import '../../atom-one.css';
import DocsLayout from '../../components/DocsLayout';
import Markdown from '../../components/Markdown';
import { Doc, DocsNavigation } from './api';

function Page(
	props:
		| { doc: Doc; navigation: DocsNavigation }
		| { data: { doc: Doc; navigation: DocsNavigation } }
) {
	const { doc, navigation } = 'data' in props ? props.data : props;
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
				</Markdown>
			</DocsLayout>
		</>
	);
}

export { Page };
