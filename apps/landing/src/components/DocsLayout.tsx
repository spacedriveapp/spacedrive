import { ChevronRightIcon } from '@heroicons/react/24/solid';
import { Button } from '@sd/ui';
import { List, X } from 'phosphor-react';
import { PropsWithChildren, useState } from 'react';
import pkg from 'react-burger-menu';

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
		<div className="flex flex-col items-start w-full sm:flex-row">
			<Menu
				onClose={() => setMenuOpen(false)}
				customBurgerIcon={false}
				isOpen={menuOpen}
				pageWrapId="page-container"
				className="shadow-2xl shadow-black"
			>
				<div className="visible h-screen pb-20 overflow-x-hidden custom-scroll doc-sidebar-scroll bg-gray-650 pt-7 px-7 sm:invisible">
					<Button
						onClick={() => setMenuOpen(!menuOpen)}
						className="!px-1 -ml-0.5 mb-3 !border-none"
					>
						<X weight="bold" className="w-6 h-6" />
					</Button>
					<DocsSidebar activePath={props?.doc?.url} navigation={props.navigation} />
				</div>
			</Menu>

			<aside className="sticky hidden px-5 mt-32 mb-20 ml-2 mr-0 lg:mr-4 top-32 sm:inline">
				<DocsSidebar activePath={props?.doc?.url} navigation={props.navigation} />
			</aside>
			<div className="flex flex-col w-full sm:flex-row" id="page-container">
				<div className="h-12 px-5 flex w-full border-t border-gray-600 border-b mt-[65px] sm:hidden items-center ">
					<div className="flex sm:hidden">
						<Button onClick={() => setMenuOpen(!menuOpen)} className="!px-2 ml-1 !border-none">
							<List weight="bold" className="w-6 h-6" />
						</Button>
					</div>
					{props.doc?.url.split('/').map((item, index) => {
						if (index === 2) return null;
						return (
							<div key={index} className="flex flex-row items-center ml-2">
								<a className="px-1 text-sm">{toTitleCase(item)}</a>
								{index < 1 && <ChevronRightIcon className="w-4 h-4 ml-1 -mr-2" />}
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
