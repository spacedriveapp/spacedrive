import { PropsWithChildren } from 'react';

import { Sidebar } from './Sidebar';
import {
	MobileSidebarProvider,
	MobileSidebarWrapper,
	OpenMobileSidebarButton
} from './Sidebar/MobileSidebar';

import 'katex/dist/katex.min.css';
import '@docsearch/css';
// This must be imported after the docsearch css
import '~/styles/search.scss';

import { Breadcrumbs } from './Breadcrumbs';

export default function Layout({ children }: PropsWithChildren) {
	const sidebar = <Sidebar />;

	return (
		<MobileSidebarProvider>
			<div className="flex w-full flex-col items-start sm:flex-row">
				<MobileSidebarWrapper>{sidebar}</MobileSidebarWrapper>
				<aside className="sticky top-32 mb-20 ml-2 mr-0 mt-32 hidden px-5 sm:inline lg:mr-4">
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
