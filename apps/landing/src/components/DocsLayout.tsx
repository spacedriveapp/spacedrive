import { CaretRight, List, X } from 'phosphor-react';
import { PropsWithChildren, useState } from 'react';
import pkg from 'react-burger-menu';
import { Button } from '@sd/ui';
import { Doc, DocsNavigation, toTitleCase } from '../pages/docs/api';
import DocsSidebar from './DocsSidebar';

// this is due to a commonjs export, it fixes build, trust
const { push: Menu } = pkg;

interface Props extends PropsWithChildren {
	doc?: Doc;
	navigation: DocsNavigation;
}

export default function DocsLayout(props: Props) {
	const [menuOpen, setMenuOpen] = useState(false);

	return (
		<div className="flex w-full flex-col items-start sm:flex-row">
			<Menu
				onClose={() => setMenuOpen(false)}
				customBurgerIcon={false}
				isOpen={menuOpen}
				pageWrapId="page-container"
				className="shadow-2xl shadow-black"
			>
				<div className="custom-scroll doc-sidebar-scroll bg-gray-650 visible h-screen overflow-x-hidden px-7 pb-20 pt-7 sm:invisible">
					<Button
						onClick={() => setMenuOpen(!menuOpen)}
						className="-ml-0.5 mb-3 !border-none !px-1"
					>
						<X weight="bold" className="h-6 w-6" />
					</Button>
					<DocsSidebar activePath={props?.doc?.url} navigation={props.navigation} />
				</div>
			</Menu>

			<aside className="sticky top-32 mt-32 mb-20 ml-2 mr-0 hidden px-5 sm:inline lg:mr-4">
				<DocsSidebar activePath={props?.doc?.url} navigation={props.navigation} />
			</aside>
			<div className="flex w-full flex-col sm:flex-row" id="page-container">
				<div className="mt-[65px] flex h-12 w-full items-center border-y border-gray-600 px-5 sm:hidden">
					<div className="flex sm:hidden">
						<Button onClick={() => setMenuOpen(!menuOpen)} className="ml-1 !border-none !px-2">
							<List weight="bold" className="h-6 w-6" />
						</Button>
					</div>
					{props.doc?.url.split('/').map((item, index) => {
						if (index === 2) return null;
						return (
							<div key={index} className="ml-2 flex flex-row items-center">
								<a className="px-1 text-sm">{toTitleCase(item)}</a>
								{index < 1 && <CaretRight className="ml-1 -mr-2 h-4 w-4" />}
							</div>
						);
					})}
				</div>
				<div className="mx-4 overflow-x-hidden sm:mx-auto">{props.children}</div>
				<div className="w-0 sm:w-32 lg:w-64" />
			</div>
		</div>
	);
}
