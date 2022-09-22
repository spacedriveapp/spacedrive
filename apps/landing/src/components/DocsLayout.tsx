import { Disclosure, Transition } from '@headlessui/react';
import { ChevronRightIcon, XMarkIcon } from '@heroicons/react/24/solid';
import { Button } from '@sd/ui';
import clsx from 'clsx';
import { List, X } from 'phosphor-react';
import { PropsWithChildren, useEffect, useState } from 'react';
import pkg from 'react-burger-menu';

import { Doc, DocsNavigation, toTitleCase } from '../pages/docs/api';
import DocsSidebar from './DocsSidebar';

// this is due to a commonjs export, it fixes build, trust
const { slide: Menu } = pkg;

interface Props extends PropsWithChildren {
	doc?: Doc;
	navigation: DocsNavigation;
}

export default function DocsLayout(props: Props) {
	const [menuOpen, setMenuOpen] = useState(false);

	return (
		<div className={clsx('flex flex-col  items-start w-full sm:flex-row')}>
			<Menu customBurgerIcon={false} isOpen={menuOpen} pageWrapId="page-view">
				<div className="h-screen pb-20 overflow-x-hidden pt-7 bg-gray-950 px-7">
					<Button
						onClick={() => setMenuOpen(!menuOpen)}
						icon={<X weight="bold" className="w-6 h-6" />}
						className="!px-1 -ml-0.5 mb-3 !border-none"
					/>
					<DocsSidebar activePath={props?.doc?.url} navigation={props.navigation} />
				</div>
			</Menu>
			{/* {menuOpen && <div className="absolute w-screen h-screen " />} */}
			<div className="h-12 px-5 flex w-full border-t border-gray-600 border-b mt-[65px] sm:hidden  items-center ">
				<div className="flex sm:hidden">
					<Button
						onClick={() => setMenuOpen(!menuOpen)}
						icon={<List weight="bold" className="w-6 h-6" />}
						className="!px-2 ml-1 !border-none"
					/>
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
			<aside className="sticky hidden px-4 mt-32 mb-20 sm:block top-32">
				<DocsSidebar activePath={props?.doc?.url} navigation={props.navigation} />
			</aside>

			<div className="w-full px-4">{props.children}</div>
		</div>
	);
}
