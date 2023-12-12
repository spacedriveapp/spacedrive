import { CaretRight, FunnelSimple, Icon, Plus } from '@phosphor-icons/react';
import { IconTypes } from '@sd/assets/util';
import clsx from 'clsx';
import { memo, PropsWithChildren, useDeferredValue, useState } from 'react';
import { useLibraryMutation } from '@sd/client';
import {
	Button,
	ContextMenuDivItem,
	DropdownMenu,
	Input,
	Popover,
	RadixCheckbox,
	tw,
	usePopover
} from '@sd/ui';
import { useIsDark, useKeybind } from '~/hooks';

import { AppliedFilters } from './AppliedFilters';
import { useSearchContext } from './context';
import { filterRegistry, SearchFilterCRUD, useToggleOptionSelected } from './Filters';
import {
	getSearchStore,
	useRegisterSearchFilterOptions,
	useSearchRegisteredFilters,
	useSearchStore
} from './store';
import { UseSearch } from './useSearch';
import { RenderIcon } from './util';

export * from './useSearch';
export * from './context';

// const Label = tw.span`text-ink-dull mr-2 text-xs`;
export const OptionContainer = tw.div`flex flex-row items-center`;

interface SearchOptionItemProps extends PropsWithChildren {
	selected?: boolean;
	setSelected?: (selected: boolean) => void;
	icon?: Icon | IconTypes | string;
}
const MENU_STYLES = `!rounded-md border !border-app-line !bg-app-box`;

// One component so all items have the same styling, including the submenu
const SearchOptionItemInternals = (props: SearchOptionItemProps) => {
	return (
		<div className="flex w-full items-center justify-between gap-1.5">
			<div className="flex items-center gap-1.5">
				<RenderIcon icon={props.icon} />
				<span className="w-fit max-w-[150px] truncate">{props.children}</span>
			</div>
			{props.selected !== undefined && <RadixCheckbox checked={props.selected} />}
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

export const SearchOptionSubMenu = (
	props: SearchOptionItemProps & { name?: string; className?: string }
) => {
	return (
		<DropdownMenu.SubMenu
			trigger={
				<ContextMenuDivItem rightArrow variant="dull">
					<SearchOptionItemInternals {...props}>{props.name}</SearchOptionItemInternals>
				</ContextMenuDivItem>
			}
			className={clsx(MENU_STYLES, 'explorer-scroll -mt-1.5', props.className)}
		>
			{props.children}
		</DropdownMenu.SubMenu>
	);
};

export const Separator = () => <DropdownMenu.Separator className="!border-app-line" />;

const SearchOptions = ({ allowExit, children }: { allowExit?: boolean } & PropsWithChildren) => {
	const search = useSearchContext();
	const isDark = useIsDark();
	return (
		<div
			onMouseEnter={() => {
				getSearchStore().interactingWithSearchOptions = true;
			}}
			onMouseLeave={() => {
				getSearchStore().interactingWithSearchOptions = false;
			}}
			className={clsx(
				'flex h-[45px] w-full flex-row items-center',
				'gap-4 px-4',
				isDark ? 'bg-black/10' : 'bg-black/5'
			)}
		>
			{/* <OptionContainer className="flex flex-row items-center">
				<FilterContainer>
					<InteractiveSection>Paths</InteractiveSection>
				</FilterContainer>
			</OptionContainer> */}

			<AddFilterButton />

			{/* We're keeping AppliedOptions to the right of the "Add Filter" button because
				its not worth rebuilding the dropdown with custom logic to lock the position
				as the trigger will move if to the right of the applied options and that is bad UX. */}
			<div className="relative flex h-full flex-1 items-center overflow-hidden">
				<AppliedFilters />
			</div>

			{children ?? (
				<>
					{(search.dynamicFilters.length > 0 || search.search !== '') && (
						<SaveSearchButton />
					)}

					<EscapeButton />
				</>
			)}
		</div>
	);
};

export default SearchOptions;

const SearchResults = memo(
	({ searchQuery, search }: { searchQuery: string; search: UseSearch }) => {
		const { allFiltersKeys } = search;
		const searchResults = useSearchRegisteredFilters(searchQuery);

		const toggleOptionSelected = useToggleOptionSelected({ search });

		return (
			<>
				{searchResults.map((option) => {
					const filter = filterRegistry.find((f) => f.name === option.type);
					if (!filter) return;

					return (
						<SearchOptionItem
							selected={allFiltersKeys?.has(option.key)}
							setSelected={(select) =>
								toggleOptionSelected({
									filter: filter as SearchFilterCRUD,
									option,
									select
								})
							}
							key={option.key}
						>
							<div className="mr-4 flex flex-row items-center gap-1.5">
								<RenderIcon icon={filter.icon} />
								<span className="text-ink-dull">{filter.name}</span>
								<CaretRight weight="bold" className="text-ink-dull/70" />
								<RenderIcon icon={option.icon} />
								{option.name}
							</div>
						</SearchOptionItem>
					);
				})}
			</>
		);
	}
);

function AddFilterButton() {
	const search = useSearchContext();
	const searchState = useSearchStore();

	const [searchQuery, setSearch] = useState('');

	const deferredSearchQuery = useDeferredValue(searchQuery);

	for (const filter of filterRegistry) {
		const options = filter
			.useOptions({ search: searchQuery })
			.map((o) => ({ ...o, type: filter.name }));

		// eslint-disable-next-line react-hooks/rules-of-hooks
		useRegisterSearchFilterOptions(filter, options);
	}

	return (
		<OptionContainer className="shrink-0">
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
					value={searchQuery}
					onChange={(e) => setSearch(e.target.value)}
					autoFocus
					autoComplete="off"
					autoCorrect="off"
					variant="transparent"
					placeholder="Filter..."
				/>
				<Separator />
				{searchQuery === '' ? (
					filterRegistry.map((filter) => (
						<filter.Render
							key={filter.name}
							filter={filter as any}
							options={searchState.filterOptions.get(filter.name)!}
							search={search}
						/>
					))
				) : (
					<SearchResults searchQuery={deferredSearchQuery} search={search} />
				)}
			</DropdownMenu.Root>
		</OptionContainer>
	);
}

function SaveSearchButton() {
	const search = useSearchContext();
	const popover = usePopover();

	const [name, setName] = useState('');

	const saveSearch = useLibraryMutation('search.saved.create');

	return (
		<Popover
			popover={popover}
			className={MENU_STYLES}
			trigger={
				<Button className="flex shrink-0 flex-row" size="xs" variant="dotted">
					<Plus weight="bold" className="mr-1" />
					Save Search
				</Button>
			}
		>
			<form
				className="mx-1.5 my-1 flex flex-row items-center overflow-hidden"
				onSubmit={(e) => {
					e.preventDefault();
					if (!name) return;

					saveSearch.mutate({
						name,
						search: search.search,
						filters: JSON.stringify(search.mergedFilters.map((f) => f.arg)),
						description: null,
						icon: null
					});
					setName('');
					popover.setOpen(false);
				}}
			>
				<Input
					value={name}
					onChange={(e) => setName(e.target.value)}
					autoFocus
					variant="default"
					placeholder="Name"
					className="w-[130px]"
				/>
				<Button
					disabled={name.length === 0}
					type="submit"
					className="ml-2"
					variant="accent"
				>
					Save
				</Button>
			</form>
		</Popover>
	);
}

function EscapeButton() {
	const search = useSearchContext();

	useKeybind(['Escape'], () => {
		search.setSearch('');
		search.setOpen(false);
	});

	return (
		<kbd
			onClick={() => {
				search.setSearch('');
				search.setOpen(false);
			}}
			className="ml-2 rounded-lg border border-app-line bg-app-box px-2 py-1 text-[10.5px] tracking-widest shadow"
		>
			ESC
		</kbd>
	);
}
