import React from 'react';
import { Helmet } from 'react-helmet';
import { ReactComponent as Content } from '~/docs/architecture/distributed-data-sync.md';

import Markdown from '../../../components/Markdown';

function Page() {
	return (
		<Markdown>
			<Helmet>
				<title>Distributed Data Sync - Spacedrive Documentation</title>
				<meta
					name="description"
					content="How we handle data sync with SQLite in a distributed network."
				/>
			</Helmet>
			<Content />
		</Markdown>
	);
}

export { Page };
