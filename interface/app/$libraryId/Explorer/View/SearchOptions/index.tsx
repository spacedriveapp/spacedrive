import { CaretRight, FunnelSimple, Icon, Plus } from '@phosphor-icons/react';
import { IconTypes } from '@sd/assets/util';
import clsx from 'clsx';
import { PropsWithChildren, useDeferredValue, useMemo, useState } from 'react';
import { snapshot } from 'valtio';
import { Button, ContextMenuDivItem, DropdownMenu, Input, RadixCheckbox, tw } from '@sd/ui';
import { useKeybind } from '~/hooks';

import { AppliedOptions } from './AppliedFilters';
import { filterRegistry } from './Filters';
import { useSavedSearches } from './SavedSearches';
import {
	getSearchStore,
	selectFilterOption,
	updateFilterArgs,
	useRegisterSearchFilterOptions,
	useSearchRegisteredFilters,
	useSearchStore
} from './store';
import { RenderIcon } from './util';

// const Label = tw.span`text-ink-dull mr-2 text-xs`;
const OptionContainer = tw.div`flex flex-row items-center`;

interface SearchOptionItemProps extends PropsWithChildren {
	selected?: boolean;
	setSelected?: (selected: boolean) => void;
	icon?: Icon | IconTypes | string;
}
const MENU_STYLES = `!rounded-md border !border-app-line !bg-app-box`;

// One component so all items have the same styling, including the submenu
const SearchOptionItemInternals = (props: SearchOptionItemProps) => {
	return (
		<div className="flex flex-row gap-2">
			{props.selected !== undefined && (
				<RadixCheckbox checked={props.selected} onCheckedChange={props.setSelected} />
			)}
			<RenderIcon icon={props.icon} />
			{props.children}
		</div>
	);
};

// for individual items in a submenu, defined in Options
export const SearchOptionItem = (props: SearchOptionItemProps) => {
	return (
		<DropdownMenu.Item
			onSelect={(event) => {
				event.preventDefault();
				props.setSelected?.(!props.selected);
			}}
			variant="dull"
		>
			<SearchOptionItemInternals {...props} />
		</DropdownMenu.Item>
	);
};

export const SearchOptionSubMenu = (props: SearchOptionItemProps & { name?: string }) => {
	return (
		<DropdownMenu.SubMenu
			trigger={
				<ContextMenuDivItem rightArrow variant="dull">
					<SearchOptionItemInternals {...props}>{props.name}</SearchOptionItemInternals>
				</ContextMenuDivItem>
			}
			className={clsx(MENU_STYLES, '-mt-1.5')}
		>
			{props.children}
		</DropdownMenu.SubMenu>
	);
};

export const Separator = () => <DropdownMenu.Separator className="!border-app-line" />;

const SearchOptions = () => {
	const searchState = useSearchStore();

	const [newFilterName, setNewFilterName] = useState('');
	const [searchValue, setSearchValue] = useState('');

	const deferredSearchValue = useDeferredValue(searchValue);

	useKeybind(['Escape'], () => {
		getSearchStore().isSearching = false;
	});

	const savedSearches = useSavedSearches();

	const filtersWithOptions = filterRegistry.map((filter) => {
		const options = filter
			.useOptions({ search: deferredSearchValue })
			.map((o) => ({ ...o, type: filter.name }));

		// eslint-disable-next-line react-hooks/rules-of-hooks
		useRegisterSearchFilterOptions(filter, options);

		return [filter, options] as const;
	});

	const searchResults = useSearchRegisteredFilters(deferredSearchValue);

	return (
		<div
			onMouseEnter={() => {
				getSearchStore().interactingWithSearchOptions = true;
			}}
			onMouseLeave={() => {
				getSearchStore().interactingWithSearchOptions = false;
			}}
			className="flex h-[45px] w-full flex-row items-center gap-4 border-b border-app-line/50 bg-app-darkerBox/90 px-4 backdrop-blur"
		>
			{/* <OptionContainer className="flex flex-row items-center">
				<FilterContainer>
					<InteractiveSection>Paths</InteractiveSection>
				</FilterContainer>
			</OptionContainer> */}

			<OptionContainer>
				<DropdownMenu.Root
					onKeyDown={(e) => e.stopPropagation()}
					className={MENU_STYLES}
					trigger={
						<Button className="flex flex-row gap-1" size="xs" variant="dotted">
							<FunnelSimple />
							Add Filter
						</Button>
					}
				>
					<Input
						value={searchValue}
						onChange={(e) => setSearchValue(e.target.value)}
						autoFocus
						autoComplete="off"
						autoCorrect="off"
						variant="transparent"
						placeholder="Filter..."
					/>
					<Separator />
					{searchValue
						? searchResults.map((option) => {
								const filter = filterRegistry.find((f) => f.name === option.type);
								if (!filter) return;

								return (
									<SearchOptionItem
										selected={searchState.filterArgsKeys.has(option.key)}
										setSelected={(value) => {
											const searchStore = getSearchStore();

											updateFilterArgs((args) => {
												if (searchStore.fixedFilterKeys.has(option.key))
													return;

												let rawArg = args.find((arg) => filter.find(arg));

												if (!rawArg) {
													rawArg = filter.create(option.value);
													args.push(rawArg);
												}

												const rawArgIndex = args.findIndex((arg) =>
													filter.find(arg)
												)!;

												const arg = filter.find(rawArg)! as any;

												if (!filter.getCondition?.(arg))
													filter.setCondition(
														arg,
														Object.keys(filter.conditions)[0] as any
													);

												if (value) filter.applyAdd(arg, option);
												else {
													if (!filter.applyRemove(arg, option))
														args.splice(rawArgIndex);
												}
											});
										}}
										key={option.key}
									>
										<div className="mr-4 flex flex-row items-center gap-1.5">
											<RenderIcon icon={option.icon} />
											<span className="text-ink-dull">{filter.name}</span>
											<CaretRight
												weight="bold"
												className="text-ink-dull/70"
											/>
											<RenderIcon icon={option.icon} />
											{option.name}
										</div>
									</SearchOptionItem>
								);
						  })
						: filtersWithOptions.map(([filter, options]) => (
								<filter.Render
									key={filter.name}
									filter={filter as any}
									options={options}
								/>
						  ))}
				</DropdownMenu.Root>
			</OptionContainer>
			{/* We're keeping AppliedOptions to the right of the "Add Filter" button because its not worth rebuilding the dropdown with custom logic to lock the position as the trigger will move if to the right of the applied options and that is bad UX. */}
			<AppliedOptions />
			<div className="grow" />

			{searchState.selectedFilters.size > 0 && (
				<DropdownMenu.Root
					className={clsx(MENU_STYLES)}
					trigger={
						<Button className="flex flex-row" size="xs" variant="dotted">
							<Plus weight="bold" className="mr-1" />
							Save Search
						</Button>
					}
				>
					<div className="mx-1.5 my-1 flex flex-row items-center overflow-hidden">
						<Input
							value={newFilterName}
							onChange={(e) => setNewFilterName(e.target.value)}
							autoFocus
							variant="default"
							placeholder="Name"
							className="w-[130px]"
						/>
						<Button
							onClick={() => {
								if (!newFilterName) return;
								savedSearches.saveSearch(newFilterName);
								setNewFilterName('');
							}}
							className="ml-2"
							variant="accent"
						>
							Save
						</Button>
					</div>
				</DropdownMenu.Root>
			)}

			<kbd
				onClick={() => (getSearchStore().isSearching = false)}
				className="ml-2 rounded-lg border border-app-line bg-app-box px-2 py-1 text-[10.5px] tracking-widest shadow"
			>
				ESC
			</kbd>
		</div>
	);
};

export default SearchOptions;
