import { ObjectKind, useLibraryQuery } from '@sd/client';
import { getSearchStore, useCreateSearchFilter, useSearchStore } from '~/hooks';

import { FilterInput, SearchOptionItem, SearchOptionSubMenu, Separator } from '.';
import { getIconComponent } from './util';

export function LocationOptions() {
	const locationsQuery = useLibraryQuery(['locations.list'], { keepPreviousData: true });
	const searchStore = useSearchStore();

	const filterCategory = useCreateSearchFilter({
		name: 'Location',
		icon: 'Folder',
		filters:
			locationsQuery.data?.map((location) => ({
				id: location.id,
				icon: 'Folder',
				name: location.name || ''
			})) || []
	});

	return (
		<SearchOptionSubMenu
			name={filterCategory.name}
			icon={getIconComponent(filterCategory.icon)}
		>
			<FilterInput />
			<Separator />
			{filterCategory.filters.map((filter) => (
				<SearchOptionItem
					selected={searchStore.selectedFilters.has(filter.key)}
					setSelected={(value) =>
						value
							? getSearchStore().selectFilter(filter.key, true)
							: getSearchStore().deselectFilter(filter.key)
					}
					key={filter.key}
					icon="Folder"
				>
					{filter.name}
				</SearchOptionItem>
			))}
		</SearchOptionSubMenu>
	);
}

export function TagOptions() {
	const searchStore = useSearchStore();
	const tagsQuery = useLibraryQuery(['tags.list'], { keepPreviousData: true });

	const filterCategory = useCreateSearchFilter({
		name: 'Tagged',
		icon: 'CircleDashed',
		filters:
			tagsQuery.data?.map((tag) => ({
				id: tag.id,
				name: tag.name || '',
				icon: tag.color as any
			})) || []
	});

	return (
		<SearchOptionSubMenu
			name={filterCategory.name}
			icon={getIconComponent(filterCategory.icon)}
		>
			<FilterInput />
			<Separator />
			{filterCategory.filters.map((filter) => (
				<SearchOptionItem
					selected={searchStore.selectedFilters.has(filter.key)}
					setSelected={(value) =>
						value
							? getSearchStore().selectFilter(filter.key, true)
							: getSearchStore().deselectFilter(filter.key)
					}
					key={filter.id}
					icon={filter.icon}
				>
					{filter.name}
				</SearchOptionItem>
			))}
		</SearchOptionSubMenu>
	);
}
export function KindOptions() {
	const searchStore = useSearchStore();

	const filterCategory = useCreateSearchFilter({
		name: 'Kind',
		icon: 'CircleDashed',
		filters:
			Object.keys(ObjectKind)
				.filter((key) => !isNaN(Number(key)) && ObjectKind[Number(key)] !== undefined)
				.map((key) => {
					const kind = ObjectKind[Number(key)];
					return {
						id: Number(key),
						name: kind,
						icon: kind || 'defaultIcon' // providing a default icon in case kind is undefined
					};
				}) || []
	});

	return (
		<SearchOptionSubMenu
			name={filterCategory.name}
			icon={getIconComponent(filterCategory.icon)}
		>
			<FilterInput />
			<Separator />
			{filterCategory.filters.map((filter) => (
				<SearchOptionItem
					selected={searchStore.selectedFilters.has(filter.key)}
					setSelected={(value) =>
						value
							? getSearchStore().selectFilter(filter.key, true)
							: getSearchStore().deselectFilter(filter.key)
					}
					key={filter.key}
					icon={filter.icon}
				>
					{filter.name}
				</SearchOptionItem>
			))}
		</SearchOptionSubMenu>
	);
}
