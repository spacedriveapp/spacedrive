import { Button } from '@sd/ui';
import { SmileyXEyes } from 'phosphor-react';
import { Helmet } from 'react-helmet';

import Markdown from '../components/Markdown';
import { getWindow } from '../utils';

export { Page };

function Page({ is404 }: { is404: boolean }) {
	return (
		<>
			<Markdown classNames="flex w-full justify-center">
				<Helmet>
					<title>Not Found - Spacedrive</title>
				</Helmet>
				<div className="flex flex-col items-center m-auto ">
					<div className="h-32" />
					<SmileyXEyes className="mb-3 w-44 h-44" />
					<h1 className="mb-2 text-center">In the quantum realm this page potentially exists.</h1>
					<p>In other words, thats a 404.</p>
					<div className="flex flex-wrap justify-center">
						<Button
							href={getWindow()?.document.referrer || 'javascript:history.back()'}
							className="mt-2 mr-3 cursor-pointer "
							variant="gray"
						>
							← Back
						</Button>
						<Button href="/" className="mt-2 cursor-pointer !text-white" variant="accent">
							Discover Spacedrive →
						</Button>
					</div>
				</div>
				<div className="h-80" />
			</Markdown>
		</>
	);
}
