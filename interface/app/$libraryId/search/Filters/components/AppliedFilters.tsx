import { MagnifyingGlass, X } from '@phosphor-icons/react';
import clsx from 'clsx';
import { forwardRef } from 'react';
import { SearchFilterArgs } from '@sd/client';
import { Dropdown, DropdownMenu, tw } from '@sd/ui';
import { useLocale } from '~/hooks';

import { SearchOptionItem, useSearchContext } from '../..';
import HorizontalScroll from '../../../overview/Layout/HorizontalScroll';
import { filterRegistry } from '../../Filters/index';
import { RenderIcon } from '../../util';
import { useFilterOptionStore } from '../store';
import { FilterOptionList } from './FilterOptionList';

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
			<RenderIcon className="size-3" icon={X} />
		</div>
	);
});

const MENU_STYLES = `!rounded-md border !border-app-line !bg-app-box`;

export const AppliedFilters = () => {
	const search = useSearchContext();

	return (
		<>
			{search.search && (
				<FilterContainer>
					<StaticSection>
						<RenderIcon className="size-4" icon={MagnifyingGlass} />
						<FilterText>{search.search}</FilterText>
					</StaticSection>
					{search.setSearch && <CloseTab onClick={() => search.setSearch?.('')} />}
				</FilterContainer>
			)}
			<div className="group w-full">
				<HorizontalScroll className="!mb-0 !pl-0">
					{search.mergedFilters?.map(({ arg, removalIndex }, index) => {
						const filter = filterRegistry.find((f) => f.extract(arg));
						if (!filter) return;
						return (
							<div className="shrink-0" key={`${filter.name}-${index}`}>
								<FilterArg
									arg={arg}
									onDelete={
										removalIndex !== null && search.setFilters
											? () => {
													search.setFilters?.((filters) => {
														filters?.splice(removalIndex, 1);

														return filters;
													});
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
	const search = useSearchContext();

	const filterStore = useFilterOptionStore();
	const { t } = useLocale();

	const filter = filterRegistry.find((f) => f.extract(arg));
	if (!filter) return;

	const activeOptions = filter.argsToFilterOptions(
		filter.extract(arg)! as any,
		filterStore.filterOptions
	);

	// get all options for this filter
	const options = filterStore.filterOptions.get(filter.name) || [];

	function isFilterDescriptionDisplayed() {
		if (filter?.translationKey === 'hidden' || filter?.translationKey === 'favorite') {
			return false;
		} else {
			return true;
		}
	}

	return (
		<FilterContainer>
			<StaticSection>
				<RenderIcon className="size-4" icon={filter.icon} />
				<FilterText>{filter.name}</FilterText>
			</StaticSection>
			{isFilterDescriptionDisplayed() && (
				<>
					<DropdownMenu.Root
						onKeyDown={(e) => e.stopPropagation()}
						className={clsx(MENU_STYLES, 'explorer-scroll max-w-fit')}
						trigger={
							<InteractiveSection className="border-l hover:bg-app-lightBox/30">
								{
									(filter.conditions as any)[
										filter.getCondition(filter.extract(arg) as any) as any
									]
								}
							</InteractiveSection>
						}
					>
						<SearchOptionItem>Is</SearchOptionItem>
						<SearchOptionItem>Is Not</SearchOptionItem>
					</DropdownMenu.Root>
					<DropdownMenu.Root
						onKeyDown={(e) => e.stopPropagation()}
						className={clsx(MENU_STYLES, 'explorer-scroll max-w-fit')}
						trigger={
							<InteractiveSection className="gap-1 border-l border-app-darkerBox/70 py-0.5 pl-1.5 pr-2 text-sm hover:bg-app-lightBox/30">
								{activeOptions && (
									<>
										{activeOptions.length === 1 ? (
											<RenderIcon
												className="size-4"
												icon={activeOptions[0]!.icon}
											/>
										) : (
											<div className="relative flex gap-0.5 self-center">
												{activeOptions.map((option, index) => (
													<div
														key={index}
														style={{
															zIndex: activeOptions.length - index
														}}
													>
														<RenderIcon
															className="size-4"
															icon={option.icon}
														/>
													</div>
												))}
											</div>
										)}
										<span className="max-w-[150px] truncate">
											{activeOptions.length > 1
												? `${activeOptions.length} ${t(`${filter.translationKey}`, { count: activeOptions.length })}`
												: activeOptions[0]?.name}
										</span>
									</>
								)}
							</InteractiveSection>
						}
					>
						<FilterOptionList filter={filter} options={options} search={search} />
					</DropdownMenu.Root>
				</>
			)}

			{onDelete && <CloseTab onClick={onDelete} />}
		</FilterContainer>
	);
}
