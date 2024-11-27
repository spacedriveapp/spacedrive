'use client';

import { DocSearch } from '@docsearch/react';

import { useMenuContext } from './MobileSidebar';

export function SearchBar() {
	const menu = useMenuContext();

	return (
		<div
			className="mb-5"
			onClick={() => {
				if (menu.open) menu.setOpen(false);
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
