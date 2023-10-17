'use client';

import { SearchInput } from '@sd/ui';

export function SearchBar() {
	return (
		<div onClick={() => alert('Search coming soon...')} className="mb-5">
			<SearchInput
				placeholder="Search..."
				disabled
				right={<span className="pr-2 text-xs font-semibold text-gray-400">⌘K</span>}
			/>
		</div>
	);
}
