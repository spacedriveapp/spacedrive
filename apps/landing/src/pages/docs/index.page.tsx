import { Helmet } from 'react-helmet';
import DocsLayout from '../../components/DocsLayout';
import Markdown from '../../components/Markdown';
import { DocsNavigation } from './api';

function Page({ navigation }: { navigation: DocsNavigation }) {
	return (
		<>
			<Helmet>
				<title>Spacedrive Docs</title>
				<meta name="description" content="Learn more about Spacedrive" />
			</Helmet>

			<DocsLayout navigation={navigation}>
				<Markdown>
					<div className="mt-[105px]">
						<h1 className="text-4xl font-bold">Spacedrive Docs</h1>
						<p className="text-lg text-gray-400">
							Welcome to the Spacedrive documentation. Here you can find all the information you
							need to get started with Spacedrive.
						</p>
						<a
							className="transition text-primary-600 hover:text-primary-500"
							href="/docs/product/getting-started/introduction"
						>
							Get Started →
						</a>
					</div>
				</Markdown>
			</DocsLayout>
		</>
	);
}

export { Page };
