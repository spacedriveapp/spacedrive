'use client';

import { DocSearch } from '@docsearch/react';

import '@docsearch/css';
import '~/styles/search.scss';

import { useMenuContext } from './MobileSidebar';

export function SearchBar() {
	const menu = useMenuContext();

	return (
		<div
			className="mb-5"
			onClick={() => {
				menu.open && menu.setOpen(false);
			}}
		>
			<DocSearch
				appId="O2QT1W1OHH"
				apiKey="644931a3b4382b641270dd6e4d24012b"
				indexName="spacedrivedocs"
				placeholder="Search..."
			/>
		</div>
	);
}
