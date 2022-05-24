import React from 'react';
import { Helmet } from 'react-helmet';
import { ReactComponent as Content } from '~/docs/product/roadmap.md';

import { ReactComponent as Folder } from '../../../../packages/interface/src/assets/svg/folder.svg';
import Markdown from '../components/Markdown';

function Page() {
	return (
		<Markdown>
			<Helmet>
				<title>Roadmap - Spacedrive</title>
				<meta name="description" content="What can Spacedrive do?" />
			</Helmet>
			<div className="w-24 mb-10">
				<Folder className="" />
			</div>
			<Content />
		</Markdown>
	);
}

export default Page;
