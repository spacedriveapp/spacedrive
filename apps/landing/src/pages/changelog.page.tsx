import React from 'react';
import { Helmet } from 'react-helmet';
import { ReactComponent as Content } from '~/docs/changelog/index.md';

import Markdown from '../components/Markdown';

function Page() {
	return (
		<Markdown>
			<Helmet>
				<title>Changelog - Spacedrive</title>
				<meta name="description" content="Updates and release builds of the Spacedrive app." />
			</Helmet>
			<Content />
		</Markdown>
	);
}

export { Page };
