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
				apiKey="765d32dcfd1971b2b21cea6cc343e259"
				indexName="spacedrivedocs"
				placeholder="Search"
			/>
		</div>
	);
}
