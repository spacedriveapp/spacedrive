import { FunnelSimple, Icon, Plus } from '@phosphor-icons/react';
import { IconTypes } from '@sd/assets/util';
import clsx from 'clsx';
import { PropsWithChildren, useMemo, useState } from 'react';
import { Button, ContextMenuDivItem, DropdownMenu, Input, RadixCheckbox, tw } from '@sd/ui';
import { useKeybind } from '~/hooks';

import { AppliedOptions } from './AppliedFilters';
import { filterTypeRegistry } from './Filters';
import { useSavedSearches } from './SavedSearches';
import { getSearchStore, searchRegisteredFilters, useSearchStore } from './store';
import { RenderIcon } from './util';

const Label = tw.span`text-ink-dull mr-2 text-xs`;
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

export const FilterInput = () => {
	// talk to store
	return <Input autoFocus variant="transparent" placeholder="Filter..." />;
};

export const Separator = () => <DropdownMenu.Separator className="!border-app-line" />;

const SearchOptions = () => {
	const searchStore = useSearchStore();

	const [newFilterName, setNewFilterName] = useState<string>('');
	const [searchValue, setSearchValue] = useState<string>('');

	const searchResults = useMemo(() => {
		return searchRegisteredFilters(searchValue);
	}, [searchValue]);

	useKeybind(['Escape'], () => {
		getSearchStore().isSearching = false;
	});

	const savedSearches = useSavedSearches();

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
				<Label>Show:</Label>
				<Button
					onClick={() => (getSearchStore().searchType = 'paths')}
					size="xs"
					variant={searchStore.searchType === 'paths' ? 'accent' : 'gray'}
					rounding="left"
				>
					Paths
				</Button>
				<Button
					onClick={() => (getSearchStore().searchType = 'objects')}
					size="xs"
					variant={searchStore.searchType === 'objects' ? 'accent' : 'gray'}
					rounding="right"
				>
					Objects
				</Button>
			</OptionContainer>
			<div className="mx-1 h-[15px] w-[1px] bg-app-line" /> */}
			<OptionContainer>
				<DropdownMenu.Root
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
						variant="transparent"
						placeholder="Filter..."
					/>
					<Separator />
					{filterTypeRegistry.map(({ Render, ...filter }) => (
						<Render key={filter.name} filter={filter} />
					))}

					{searchValue ? (
						<>
							{/* {searchResults.map((result) => {
								const meta = filterMeta[result.type];
								return (
									<SearchOptionItem
										selected={searchStore.selectedFilters.has(result.key)}
										setSelected={(value) =>
											value ? selectFilter(result) : deselectFilter(result)
										}
										key={result.key}
									>
										<div className="mr-4 flex flex-row items-center gap-1.5">
											<RenderIcon icon={meta.icon} />
											<span className="text-ink-dull">
												{FilterType[result.type]}
											</span>
											<CaretRight
												weight="bold"
												className="text-ink-dull/70"
											/>
											<RenderIcon icon={result.icon} />
											{result.name}
										</div>
									</SearchOptionItem>
								);
							})} */}
						</>
					) : (
						<></>
					)}
				</DropdownMenu.Root>
			</OptionContainer>
			<AppliedOptions />

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

			<div className="grow" />
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
