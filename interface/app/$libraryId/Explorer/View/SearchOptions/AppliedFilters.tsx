import { MagnifyingGlass, X } from '@phosphor-icons/react';
import { forwardRef, useEffect, useState } from 'react';
import { tw } from '@sd/ui';

import { filterMeta } from './Filters';
import {
	deselectFilter,
	getSearchStore,
	getSelectedFiltersGrouped,
	GroupedFilters,
	SetFilter,
	useSearchStore
} from './store';
import { RenderIcon } from './util';

const InteractiveSection = tw.div`flex group flex-row items-center border-app-darkerBox/70 px-2 py-0.5 text-sm text-ink-dull hover:bg-app-lightBox/20`;
const FilterContainer = tw.div`flex flex-row items-center rounded bg-app-box overflow-hidden`;
const FilterText = tw.span`mx-1 py-0.5 text-sm text-ink-dull`;
const StaticSection = tw.div`flex flex-row items-center pl-2 pr-1 text-sm`;

const CloseTab = forwardRef<HTMLDivElement, { onClick: () => void }>(({ onClick }, ref) => {
	return (
		<div
			ref={ref}
			className="flex h-full items-center rounded-r border-l border-app-darkerBox/70 px-1.5 py-0.5 text-sm hover:bg-app-lightBox/30"
			onClick={onClick}
		>
			<RenderIcon className="h-3 w-3" icon={X} />
		</div>
	);
});

export const AppliedOptions = () => {
	const searchStore = useSearchStore();
	const [groupedFilters, setGroupedFilters] = useState<GroupedFilters[]>();

	useEffect(() => {
		setGroupedFilters(getSelectedFiltersGrouped());
	}, [searchStore, searchStore.selectedFilters.size]);

	function deselectFilters(filters: SetFilter[]) {
		filters.forEach((filter) => {
			if (filter.canBeRemoved) deselectFilter(filter);
		});
	}

	return (
		<div className="flex flex-row gap-2">
			{searchStore.searchQuery && (
				<FilterContainer>
					<StaticSection>
						<RenderIcon className="h-4 w-4" icon={MagnifyingGlass} />
						<FilterText>{searchStore.searchQuery}</FilterText>
					</StaticSection>
					<CloseTab onClick={() => (getSearchStore().searchQuery = null)} />
				</FilterContainer>
			)}
			{groupedFilters?.map((group) => {
				const showRemoveButton = group.filters.some((filter) => filter.canBeRemoved);
				const meta = filterMeta[group.type];

				return (
					<FilterContainer key={group.type}>
						<StaticSection>
							<RenderIcon className="h-4 w-4" icon={meta?.icon} />
							<FilterText>{meta?.name}</FilterText>
						</StaticSection>
						<InteractiveSection className="border-l ">
							{group.filters.length > 1
								? meta?.wording.plural
								: meta?.wording.singular}
						</InteractiveSection>

						<InteractiveSection className="gap-1 border-l border-app-darkerBox/70 py-0.5 pl-1.5 pr-2 text-sm">
							{group.filters.length > 1 && (
								<div
									className="relative"
									style={{ width: `${group.filters.length * 12}px` }}
								>
									{group.filters.map((filter, index) => (
										<div
											key={index}
											className="absolute -top-2 left-0"
											style={{
												zIndex: group.filters.length - index,
												left: `${index * 10}px`
											}}
										>
											<RenderIcon className="h-4 w-4" icon={filter.icon} />
										</div>
									))}
								</div>
							)}
							{group.filters.length === 1 && (
								<RenderIcon className="h-4 w-4" icon={group.filters[0]?.icon} />
							)}
							{group.filters.length > 1
								? `${group.filters.length} ${pluralize(meta?.name)}`
								: group.filters[0]?.name}
						</InteractiveSection>

						{showRemoveButton && (
							<CloseTab onClick={() => deselectFilters(group.filters)} />
						)}
					</FilterContainer>
				);
			})}
		</div>
	);
};

function pluralize(word?: string) {
	if (word?.endsWith('s')) return word;
	return `${word}s`;
}
