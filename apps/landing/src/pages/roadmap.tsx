import Markdown from '../components/Markdown';
import React from 'react';
import { ReactComponent as Content } from '~/docs/product/roadmap.md';
import { Helmet } from 'react-helmet';
import { ReactComponent as Folder } from '../../../../packages/interface/src/assets/svg/folder.svg';

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
