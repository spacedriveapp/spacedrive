import { Button } from '@sd/ui';
import React from 'react';
import { PageContextBuiltIn } from 'vite-plugin-ssr';

import '@sd/ui/style';

import { Footer } from './components/Footer';
import NavBar from './components/NavBar';
import { PageContextProvider } from './renderer/usePageContext';
import './style.scss';

export default function App({
	children,
	pageContext
}: {
	children: React.ReactNode;
	pageContext: PageContextBuiltIn;
}) {
	return (
		<React.StrictMode>
			<PageContextProvider pageContext={pageContext}>
				<div className="dark dark:bg-black dark:text-white overflow-x-hidden">
					<Button
						href="#content"
						className="fixed left-0 z-50 mt-3 ml-8 duration-200 -translate-y-16 cursor-pointer focus:translate-y-0"
						variant="gray"
					>
						Skip to content
					</Button>

					<NavBar />
					<div className="sm:container w-full z-10 flex flex-col items-center px-4 mx-auto overflow-x-hidden sm:overflow-x-visible ">
						{children}
						<Footer />
					</div>
				</div>
			</PageContextProvider>
		</React.StrictMode>
	);
}
