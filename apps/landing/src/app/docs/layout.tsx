import { PropsWithChildren } from 'react';

import { Sidebar } from './Sidebar';
import {
	MobileSidebarProvider,
	MobileSidebarWrapper,
	OpenMobileSidebarButton
} from './Sidebar/MobileSidebar';

import 'katex/dist/katex.min.css';
import '@docsearch/css';
import '~/styles/search.scss';

import { Breadcrumbs } from './Breadcrumbs';

export default function Layout({ children }: PropsWithChildren) {
	const sidebar = <Sidebar />;

	return (
		<MobileSidebarProvider>
			<div className="flex w-full flex-col items-start sm:flex-row">
				<MobileSidebarWrapper>{sidebar}</MobileSidebarWrapper>
				<aside className="sticky top-32 mb-20 ml-2 mr-0 mt-32 hidden rounded-xl p-5 backdrop-saturate-[1.6] sm:inline lg:mr-4">
					{/* Gradient Borders */}
					<div className="absolute right-0 top-0 h-full w-px bg-gradient-to-b from-transparent via-[#2D2D37]/60 to-transparent" />
					{sidebar}
				</aside>
				<div className="flex w-full flex-col sm:flex-row" id="page-container">
					<div className="mt-[65px] flex h-12 w-full items-center border-y border-gray-600 px-5 sm:hidden">
						<div className="flex sm:hidden">
							<OpenMobileSidebarButton />
						</div>
						<Breadcrumbs />
					</div>
					<div className="overflow-x-hidden sm:mx-auto">{children}</div>
					<div className="w-0 sm:w-32 lg:w-64" />
				</div>
			</div>
		</MobileSidebarProvider>
	);
}
