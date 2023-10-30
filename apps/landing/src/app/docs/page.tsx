import Link from 'next/link';

import { Markdown } from './Markdown';

export const metadata = {
	title: 'Spacedrive Docs',
	description: 'Learn more about Spacedrive'
};

export default function Page() {
	return (
		<Markdown>
			<div className="mt-[105px]">
				<h1 className="text-4xl font-bold">Spacedrive Docs</h1>
				<p className="text-lg text-gray-400">
					Welcome to the Spacedrive documentation. Here you can find all the information
					you need to get started with Spacedrive.
				</p>
				<Link
					className="text-primary-600 transition hover:text-primary-500"
					href="/docs/product/getting-started/introduction"
				>
					Get Started →
				</Link>
			</div>
		</Markdown>
	);
}
