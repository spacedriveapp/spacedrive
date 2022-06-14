import { Button } from '@sd/ui';
import React, { Suspense } from 'react';
import { BrowserRouter as Router, useRoutes } from 'react-router-dom';

import '@sd/ui/style';

import { Footer } from './components/Footer';
import NavBar from './components/NavBar';
import './style.scss';

export function App() {
	return (
		<Suspense fallback={<p>Loading...</p>}>
			<div className="dark:bg-black dark:text-white ">
				<Button
					href="#content"
					className="fixed left-0 z-50 mt-3 ml-8 duration-200 -translate-y-16 cursor-pointer focus:translate-y-0"
					variant="gray"
				>
					Skip to content
				</Button>

				<NavBar />
				<div className="sm:container w-full z-10 flex flex-col items-center px-4 mx-auto overflow-x-hidden sm:overflow-x-visible ">
					{useRoutes(routes)}
					<Footer />
				</div>
			</div>
		</Suspense>
	);
}
