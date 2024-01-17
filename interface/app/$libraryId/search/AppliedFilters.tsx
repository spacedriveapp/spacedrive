import { MagnifyingGlass, X } from '@phosphor-icons/react';
import { forwardRef } from 'react';
import { SearchFilterArgs } from '@sd/client';
import { tw } from '@sd/ui';

import { useSearchContext } from '.';
import HorizontalScroll from '../overview/Layout/HorizontalScroll';
import { filterRegistry } from './Filters';
import { useSearchStore } from './store';
import { RenderIcon } from './util';

export const FilterContainer = tw.div`flex flex-row items-center rounded bg-app-box overflow-hidden shrink-0 h-6`;

export const InteractiveSection = tw.div`flex group flex-row items-center border-app-darkerBox/70 px-2 py-0.5 text-sm text-ink-dull`; // hover:bg-app-lightBox/20

export const StaticSection = tw.div`flex flex-row items-center pl-2 pr-1 text-sm`;

export const FilterText = tw.span`mx-1 py-0.5 text-sm text-ink-dull`;

export const CloseTab = forwardRef<HTMLDivElement, { onClick: () => void }>(({ onClick }, ref) => {
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

export const AppliedFilters = ({ allowRemove = true }: { allowRemove?: boolean }) => {
	const search = useSearchContext();
	return (
		<>
			{search.search && (
				<FilterContainer>
					<StaticSection>
						<RenderIcon className="h-4 w-4" icon={MagnifyingGlass} />
						<FilterText>{search.search}</FilterText>
					</StaticSection>
					{allowRemove && <CloseTab onClick={() => search.setSearch('')} />}
				</FilterContainer>
			)}
			<div className="group w-full">
				<HorizontalScroll className="!mb-0 !pl-0">
					{search.mergedFilters.map(({ arg, removalIndex }, index) => {
						const filter = filterRegistry.find((f) => f.extract(arg));
						if (!filter) return;
						return (
							<div className="shrink-0" key={`${filter.name}-${index}`}>
								<FilterArg
									arg={arg}
									onDelete={
										removalIndex !== null && allowRemove
											? () => {
													search.updateDynamicFilters(
														(dyanmicFilters) => {
															dyanmicFilters.splice(removalIndex, 1);

															return dyanmicFilters;
														}
													);
												}
											: undefined
									}
								/>
							</div>
						);
					})}
				</HorizontalScroll>
			</div>
		</>
	);
};

export function FilterArg({ arg, onDelete }: { arg: SearchFilterArgs; onDelete?: () => void }) {
	const searchStore = useSearchStore();

	const filter = filterRegistry.find((f) => f.extract(arg));
	if (!filter) return;

	const activeOptions = filter.argsToOptions(
		filter.extract(arg)! as any,
		searchStore.filterOptions
	);

	return (
		<FilterContainer>
			<StaticSection>
				<RenderIcon className="h-4 w-4" icon={filter.icon} />
				<FilterText>{filter.name}</FilterText>
			</StaticSection>
			<InteractiveSection className="border-l">
				{/* {Object.entries(filter.conditions).map(([value, displayName]) => (
                            <div key={value}>{displayName}</div>
                        ))} */}
				{(filter.conditions as any)[filter.getCondition(filter.extract(arg) as any) as any]}
			</InteractiveSection>

			<InteractiveSection className="gap-1 border-l border-app-darkerBox/70 py-0.5 pl-1.5 pr-2 text-sm">
				{activeOptions && (
					<>
						{activeOptions.length === 1 ? (
							<RenderIcon className="h-4 w-4" icon={activeOptions[0]!.icon} />
						) : (
							<div className="relative flex gap-0.5 self-center">
								{activeOptions.map((option, index) => (
									<div
										key={index}
										style={{
											zIndex: activeOptions.length - index
										}}
									>
										<RenderIcon className="h-4 w-4" icon={option.icon} />
									</div>
								))}
							</div>
						)}
						<span className="max-w-[150px] truncate">
							{activeOptions.length > 1
								? `${activeOptions.length} ${pluralize(filter.name)}`
								: activeOptions[0]?.name}
						</span>
					</>
				)}
			</InteractiveSection>

			{onDelete && <CloseTab onClick={onDelete} />}
		</FilterContainer>
	);
}

function pluralize(word?: string) {
	if (word?.endsWith('s')) return word;
	return `${word}s`;
}
