import React from 'react';
import { Helmet } from 'react-helmet';
import { ReactComponent as Content } from '~/docs/changelog/index.md';

function Page() {
	return (
		<>
			<Helmet>
				<title>Careers - Spacedrive</title>
				<meta name="description" content="Work with us to build the future of file management." />
			</Helmet>
			<div className="max-w-4xl p-4 mt-32 mb-20 sm:container">
				<div id="content" className="m-auto prose lg:prose-xs dark:prose-invert">
					<h1 className="z-30 px-2 mb-3 text-3xl font-black leading-tight text-center text-white fade-in-heading md:text-5xl">
						Come with us for a drive through space.
					</h1>
				</div>
			</div>
		</>
	);
}

export default Page;
