import { Disclosure, Menu, Transition } from '@headlessui/react';
import { ChevronRightIcon } from '@heroicons/react/24/solid';
import { Button } from '@sd/ui';
import { List } from 'phosphor-react';
import { PropsWithChildren } from 'react';

import { Doc, DocsNavigation, toTitleCase } from '../pages/docs/api';
import DocsSidebar from './DocsSidebar';

interface Props extends PropsWithChildren {
	doc?: Doc;
	navigation: DocsNavigation;
}

export default function DocsLayout(props: Props) {
	return (
		<div className="flex flex-col items-start w-full sm:flex-row">
			<div className="h-12 flex w-full border-t border-gray-600 border-b mt-[65px] sm:hidden  items-center px-3">
				<div className="block sm:hidden">
					<Disclosure>
						<Disclosure.Button className="py-2">
							<Button
								icon={<List weight="bold" className="w-5 h-5" />}
								className="!p-1.5 !border-none"
							/>
						</Disclosure.Button>
						<Disclosure.Panel className="absolute top-0 left-0 h-screen pt-20 pb-2 bg-gray-900 px-7 ">
							<DocsSidebar activePath={props?.doc?.url} navigation={props.navigation} />
						</Disclosure.Panel>
					</Disclosure>
				</div>
				{props.doc?.url.split('/').map((item, index) => {
					if (index === 2) return null;
					return (
						<div key={index} className="flex flex-row items-center ml-3">
							<a className="px-1 text-sm">{toTitleCase(item)}</a>
							{index < 1 && <ChevronRightIcon className="w-4 h-4 ml-1 -mr-2" />}
						</div>
					);
				})}
			</div>
			<aside className="sticky hidden mt-32 mb-20 sm:block top-32">
				<DocsSidebar activePath={props?.doc?.url} navigation={props.navigation} />
			</aside>
			<div className="w-full">{props.children}</div>
		</div>
	);
}
