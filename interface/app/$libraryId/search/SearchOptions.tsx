import { FunnelSimple, Icon, Plus } from '@phosphor-icons/react';
import { IconTypes } from '@sd/assets/util';
import clsx from 'clsx';
import { use } from 'i18next';
import { memo, PropsWithChildren, useDeferredValue, useMemo, useState } from 'react';
import { get } from 'react-hook-form';
import { useFeatureFlag, useLibraryMutation } from '@sd/client';
import {
	Button,
	ContextMenuDivItem,
	DropdownMenu,
	Input,
	Popover,
	RadixCheckbox,
	toast,
	tw,
	usePopover
} from '@sd/ui';
import { useIsDark, useKeybind, useLocale, useShortcut } from '~/hooks';

import { getQuickPreviewStore, useQuickPreviewStore } from '../Explorer/QuickPreview/store';
import { useSearchContext } from './context';
import { AppliedFilters, InteractiveSection } from './Filters/components/AppliedFilters';
import { filterRegistry, SearchFilterCRUD, useToggleOptionSelected } from './Filters/index';
import {
	useFilterOptionStore,
	useRegisterFilterOptions,
	useSearchRegisteredFilters
} from './Filters/store';
import { getSearchStore, useSearchStore } from './store';
import { UseSearch } from './useSearch';
import { RenderIcon } from './util';

export * from './context';
export * from './useSearch';

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
			<div className="flex w-[165px] items-center gap-1.5 overflow-hidden">
				<RenderIcon icon={props.icon} />
				<span className="truncate">{props.children}</span>
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
			className={clsx(MENU_STYLES, 'default-scroll -mt-1.5 max-h-80', props.className)}
		>
			{props.children}
		</DropdownMenu.SubMenu>
	);
};

export const Separator = () => <DropdownMenu.Separator className="!border-app-line" />;

export const SearchOptions = ({
	allowExit,
	children
}: { allowExit?: boolean } & PropsWithChildren) => {
	const search = useSearchContext();
	const isDark = useIsDark();

	const showSearchTargets = useFeatureFlag('searchTargetSwitcher');

	const { t } = useLocale();

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
			{showSearchTargets && (
				<OptionContainer className="flex flex-row items-center overflow-hidden rounded">
					<InteractiveSection
						onClick={() => search.setTarget?.('paths')}
						className={clsx(
							search.target === 'paths' ? 'bg-app-box' : 'hover:bg-app-box/50'
						)}
					>
						{t('paths')}
					</InteractiveSection>
					<InteractiveSection
						onClick={() => search.setTarget?.('objects')}
						className={clsx(
							search.target === 'objects' ? 'bg-app-box' : 'hover:bg-app-box/50'
						)}
					>
						{t('objects')}
					</InteractiveSection>
				</OptionContainer>
			)}

			<AddFilterButton />

			{/* We're keeping AppliedOptions to the right of the "Add Filter" button because
				its not worth rebuilding the dropdown with custom logic to lock the position
				as the trigger will move if to the right of the applied options and that is bad UX. */}
			<div className="relative flex h-full flex-1 cursor-default items-center overflow-hidden">
				<AppliedFilters />
			</div>

			{children ?? (
				<>
					{((search.filters && search.filters.length > 0) || search.search !== '') && (
						<SaveSearchButton />
					)}

					<EscapeButton />
				</>
			)}
		</div>
	);
};

const SearchResults = memo(
	({ searchQuery, search }: { searchQuery: string; search: UseSearch<any> }) => {
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
							<div className="mr-4 flex flex-row items-center gap-2.5">
								<div className="flex items-center gap-1">
									<RenderIcon
										className="size-[13px] opacity-80"
										icon={filter.icon}
									/>
									<span className="text-xs text-ink-dull opacity-80">
										{filter.name}
									</span>
								</div>
								<div className="flex items-center gap-1 overflow-hidden">
									<RenderIcon icon={option.icon} />
									<span className="truncate">{option.name}</span>
								</div>
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
	const filterStore = useFilterOptionStore();

	const [searchQuery, setSearch] = useState('');

	const deferredSearchQuery = useDeferredValue(searchQuery);

	const registerFilters = useMemo(
		() =>
			filterRegistry.map((filter) => (
				<RegisterSearchFilterOptions
					key={filter.name}
					filter={filter}
					searchQuery={searchQuery}
				/>
			)),
		[searchQuery]
	);

	const { t } = useLocale();

	return (
		<>
			{registerFilters}
			<OptionContainer className="shrink-0">
				<DropdownMenu.Root
					onKeyDown={(e) => e.stopPropagation()}
					className={clsx(
						MENU_STYLES,
						'default-scroll max-h-[80vh] min-h-[100px] min-w-[200px] max-w-fit'
					)}
					trigger={
						<Button className="flex flex-row gap-1" size="xs" variant="dotted">
							<FunnelSimple />
							{t('add_filter')}
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
						placeholder={`${t('filter')}...`}
					/>
					<Separator />
					{searchQuery === '' ? (
						filterRegistry.map((filter) => (
							<filter.Render
								key={filter.name}
								filter={filter as any}
								options={filterStore.filterOptions.get(filter.name)!}
								search={search}
							/>
						))
					) : (
						<SearchResults searchQuery={deferredSearchQuery} search={search} />
					)}
				</DropdownMenu.Root>
			</OptionContainer>
		</>
	);
}

function SaveSearchButton() {
	const search = useSearchContext();
	const popover = usePopover();

	const [name, setName] = useState('');

	const saveSearch = useLibraryMutation('search.saved.create');

	const { t } = useLocale();

	return (
		<Popover
			popover={popover}
			className={MENU_STYLES}
			trigger={
				<Button className="flex shrink-0 flex-row" size="xs" variant="dotted">
					<Plus weight="bold" className="mr-1" />
					{t('save_search')}
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
						target: search.target,
						search: search.search,
						filters: search.mergedFilters
							? JSON.stringify(search.mergedFilters.map((f) => f.arg))
							: undefined,
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
					placeholder={t('name')}
					className="w-[130px]"
				/>
				<Button
					disabled={name.length === 0}
					type="submit"
					className="ml-2"
					variant="accent"
				>
					{t('save')}
				</Button>
			</form>
		</Popover>
	);
}

function EscapeButton() {
	const search = useSearchContext();
	let { open: isQpOpen } = useQuickPreviewStore();

	function escape() {
		search.setSearch?.(undefined);
		search.setFilters?.(undefined);
		search.setSearchBarFocused(false);
	}

	useShortcut('explorerEscape', (e) => {
		isQpOpen = getQuickPreviewStore().open;

		e.preventDefault();
		e.stopPropagation();
		// Check the open state from the store
		if (!isQpOpen) {
			escape();
		}
	});

	return (
		<kbd
			onClick={escape}
			className="ml-2 rounded-lg border border-app-line bg-app-box px-2 py-1 text-[10.5px] tracking-widest shadow"
		>
			ESC
		</kbd>
	);
}

function RegisterSearchFilterOptions(props: {
	filter: (typeof filterRegistry)[number];
	searchQuery: string;
}) {
	const options = props.filter.useOptions({ search: props.searchQuery });

	useRegisterFilterOptions(
		props.filter,
		useMemo(
			() => options.map((o) => ({ ...o, type: props.filter.name })),
			[options, props.filter]
		)
	);

	return null;
}
