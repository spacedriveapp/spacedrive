import {
	Clock,
	Cube,
	Devices,
	FilePlus,
	FunnelSimple,
	Icon,
	Image,
	Key,
	SelectionSlash,
	User
} from '@phosphor-icons/react';
import { IconTypes } from '@sd/assets/util';
import clsx from 'clsx';
import { PropsWithChildren, useState } from 'react';
import {
	Button,
	ContextMenuDivItem,
	DropdownMenu,
	Input,
	RadixCheckbox,
	Select,
	SelectOption,
	tw
} from '@sd/ui';
import { useKeybind } from '~/hooks';

import { AppliedOptions } from './AppliedFilters';
// import { KindFilter, LocationFilter, TagFilter } from './Filters';
import { KindFilter, LocationFilter, TagFilter } from './Filters';
import { getSearchStore, useSearchStore } from './store';
import { RenderIcon } from './util';

const Label = tw.span`text-ink-dull mr-2 text-xs`;
const OptionContainer = tw.div`flex flex-row items-center`;

// type DateOption = 'before' | 'after' | 'exactly' | 'today' | 'within_last';

// type CustomFilterOptions = 'none' | 'created_date' | 'modified_date' | 'indexed_date';

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

	const handleMouseEnter = () => {
		getSearchStore().interactingWithSearchOptions = true;
	};

	const handleMouseLeave = () => {
		getSearchStore().interactingWithSearchOptions = false;
	};

	useKeybind(['Escape'], () => {
		getSearchStore().isSearching = false;
	});

	return (
		<div
			onMouseEnter={handleMouseEnter}
			onMouseLeave={handleMouseLeave}
			className="flex h-[45px] w-full flex-row items-center gap-4 border-b border-app-line/50 bg-app-darkerBox/90 px-4 backdrop-blur"
		>
			<OptionContainer className="flex flex-row items-center">
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
			{/* <OptionContainer>
				<Label>In:</Label>
				<Select
					size="sm"
					className="w-[130px]"
					onChange={(scope) => (getSearchStore().searchScope = scope)}
					value={searchStore.searchScope}
				>
					<SelectOption value="directory">This Directory</SelectOption>
					<SelectOption value="location">This Location</SelectOption>
					<SelectOption value="device">This Device</SelectOption>
					<SelectOption value="library">Entire Library</SelectOption>
				</Select>
			</OptionContainer> */}
			<div className="mx-1 h-[15px] w-[1px] bg-app-line" />

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
					<Input autoFocus variant="transparent" placeholder="Filter..." />
					<Separator />
					<LocationFilter />
					<KindFilter />
					<SearchOptionItem icon={Cube}>Size</SearchOptionItem>
					<TagFilter />
					<SearchOptionItem icon={FilePlus}>In File Contents</SearchOptionItem>
					<SearchOptionItem icon={Image}>In Album</SearchOptionItem>
					<SearchOptionItem icon={Devices}>On Device</SearchOptionItem>
					<SearchOptionItem icon={Key}>Encrypted with Key</SearchOptionItem>
					<SearchOptionItem icon={User}>Shared by</SearchOptionItem>
					<Separator />
					<SearchOptionItem icon={Clock}>Created At</SearchOptionItem>
					<SearchOptionItem icon={Clock}>Modified At</SearchOptionItem>
					<SearchOptionItem icon={Clock}>Last Opened At</SearchOptionItem>
					<SearchOptionItem icon={Clock}>Taken At</SearchOptionItem>
					<Separator />
					<SearchOptionItem icon={SelectionSlash}>Hidden</SearchOptionItem>
				</DropdownMenu.Root>
			</OptionContainer>
			<AppliedOptions />

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
