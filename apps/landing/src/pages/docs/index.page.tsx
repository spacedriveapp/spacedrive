import { Helmet } from 'react-helmet';
import { ReactComponent as Content } from '~/docs/welcome.md';

import DocsLayout from '../../components/DocsLayout';
import Markdown from '../../components/Markdown';
import { DocsList, SidebarCategory } from './api';

function Page({ docsList }: { docsList: DocsList }) {
	return (
		<>
			<Helmet>
				<title>Spacedrive Docs</title>
				<meta name="description" content="Learn more about Spacedrive" />
			</Helmet>

			<DocsLayout docsList={docsList}>
				<Markdown>
					<div className="">
						<Content />
					</div>
				</Markdown>
			</DocsLayout>
		</>
	);
}

export { Page };
