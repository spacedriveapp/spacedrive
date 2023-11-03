import { CaretRight, FunnelSimple, Icon, Plus } from '@phosphor-icons/react';
import { IconTypes } from '@sd/assets/util';
import clsx from 'clsx';
import { PropsWithChildren, useMemo, useState } from 'react';
import { Button, ContextMenuDivItem, DropdownMenu, Input, RadixCheckbox, tw } from '@sd/ui';
import { useKeybind } from '~/hooks';

import { AppliedOptions } from './AppliedFilters';
import { filterRegistry } from './Filters';
import { useSavedSearches } from './SavedSearches';
import {
	deselectFilterOption,
	getSearchStore,
	selectFilterOption,
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
	const searchStore = useSearchStore();

	const [newFilterName, setNewFilterName] = useState('');
	const [searchValue, setSearchValue] = useState('');

	useKeybind(['Escape'], () => {
		getSearchStore().isSearching = false;
	});

	const savedSearches = useSavedSearches();

	const filtersWithOptions = filterRegistry.map((filter) => {
		const options = filter
			.useOptions({ search: searchValue })
			.map((o) => ({ ...o, type: filter.name }));

		// eslint-disable-next-line react-hooks/rules-of-hooks
		useRegisterSearchFilterOptions(filter, options);

		return [filter, options] as const;
	});

	const searchResults = useSearchRegisteredFilters(searchValue);

	return (
		<div
			onMouseEnter={() => {
				getSearchStore().interactingWithSearchOptions = true;
			}}
			onMouseLeave={() => {
				getSearchStore().interactingWithSearchOptions = false;
			}}
			className="bg-app-darkerBox/90 flex h-[45px] w-full flex-row items-center gap-4 border-b border-app-line/50 px-4 backdrop-blur"
		>
			{/* <OptionContainer className="flex flex-row items-center">
				<FilterContainer>
					<InteractiveSection>Paths</InteractiveSection>
				</FilterContainer>
			</OptionContainer> */}
			<AppliedOptions />
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
						? searchResults.map((result) => {
								const filter = filterRegistry.find((f) => f.name === result.type);
								if (!filter) return;

								return (
									<SearchOptionItem
										selected={searchStore.selectedFilters.has(result.key)}
										setSelected={(value) =>
											value
												? selectFilterOption(result)
												: deselectFilterOption(result)
										}
										key={result.key}
									>
										<div className="mr-4 flex flex-row items-center gap-1.5">
											<RenderIcon icon={filter.icon} />
											<span className="text-ink-dull">{filter.name}</span>
											<CaretRight
												weight="bold"
												className="text-ink-dull/70"
											/>
											<RenderIcon icon={result.icon} />
											{result.name}
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
			<div className="grow" />

			{searchStore.selectedFilters.size > 0 && (
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
