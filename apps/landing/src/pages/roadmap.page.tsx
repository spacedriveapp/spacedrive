import React from 'react';
import { Helmet } from 'react-helmet';
import { ReactComponent as Content } from '~/docs/product/roadmap.md';

import { Folder } from '../../../../packages/interface/src/components/icons/Folder';
import Markdown from '../components/Markdown';

function Page() {
	return (
		<Markdown>
			<Helmet>
				<title>Roadmap - Spacedrive</title>
				<meta name="description" content="What can Spacedrive do?" />
			</Helmet>
			<div className="w-24 mb-10">
				<Folder className="w-44" />
			</div>
			<Content />
		</Markdown>
	);
}

export { Page };
