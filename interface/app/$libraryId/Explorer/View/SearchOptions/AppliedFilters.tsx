import { X } from '@phosphor-icons/react';
import { useEffect, useState } from 'react';
import { Button, cva, tw } from '@sd/ui';
import { Icon } from '~/components';
import { getSearchStore, GroupedFilters, SetFilter, useSearchStore } from '~/hooks';

import { getIconComponent, RenderIcon } from './util';

// const Section = tw.div`gap-2`;
const InteractiveSection = tw.div`flex group flex-row items-center border-app-darkerBox/70 px-2 py-0.5 text-sm text-ink-dull hover:bg-app-lightBox/30`;

export const AppliedOptions = () => {
	const searchStore = useSearchStore();
	const [groupedFilters, setGroupedFilters] = useState<GroupedFilters>({});

	useEffect(() => {
		setGroupedFilters(getSearchStore().getSelectedFilters());
	}, [searchStore, searchStore.selectedFilters.size]);

	function deselectFilters(filters: SetFilter[]) {
		filters.forEach((filter) => {
			getSearchStore().deselectFilter(filter.key);
		});
	}

	return (
		<div className="flex flex-row gap-2">
			{Object.entries(groupedFilters).map(([categoryName, filters]) => (
				<div className="flex flex-row items-center rounded bg-app-box" key={categoryName}>
					<div className="flex flex-row items-center pl-2 pr-1 text-sm">
						<RenderIcon
							className="h-4 w-4"
							icon={getIconComponent(filters[0]?.category?.icon || '') as any}
						/>
						<span className="mx-1 py-0.5 text-sm">{categoryName}</span>
					</div>
					<InteractiveSection className="border-l bg-app-lightBox/20">
						in
					</InteractiveSection>
					<InteractiveSection className="gap-1 border-l border-app-darkerBox/70 py-0.5 pl-1.5 pr-2 text-sm">
						{filters.length > 1 && (
							<div className="relative" style={{ width: `${filters.length * 13}px` }}>
								{filters.map((filter, index) => (
									<div
										key={index}
										className="absolute -top-2 left-0"
										style={{
											zIndex: filters.length - index,
											left: `${index * 10}px`
										}}
									>
										<RenderIcon className="h-4 w-4" icon={filter.icon} />
									</div>
								))}
							</div>
						)}
						{filters.length === 1 && (
							<RenderIcon className="h-4 w-4" icon={filters[0]?.icon} />
						)}
						{filters[0]?.name}
					</InteractiveSection>
					<div
						onClick={() => deselectFilters(filters)}
						className="flex h-full items-center rounded-r border-l border-app-darkerBox/70 px-1.5 py-0.5 text-sm hover:bg-app-lightBox/30"
					>
						<X weight="bold" className="opacity-50" />
					</div>
				</div>
			))}
		</div>
	);
};
