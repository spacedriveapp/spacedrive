import { CircleDashed, Folder, Tag } from '@phosphor-icons/react';
import { ObjectKind, useLibraryQuery } from '@sd/client';
import { getSearchStore, useSearchOption, useSearchStore } from '~/hooks';

import { FilterInput, SearchOptionItem, SearchOptionSubMenu, Separator } from '.';

export function LocationOptions() {
	const locationsQuery = useLibraryQuery(['locations.list'], { keepPreviousData: true });
	const searchStore = useSearchStore();

	const searchOption = useSearchOption({
		name: 'In Location',
		icon: Folder,
		options:
			locationsQuery.data?.map((location) => ({
				key: `location-${location.id}`,
				name: location.name || ''
			})) || []
	});

	return (
		<SearchOptionSubMenu name={searchOption.name} icon={searchOption.icon}>
			<FilterInput searchOption={searchOption} />
			<Separator />
			{searchOption.options.map((option) => (
				<SearchOptionItem
					selected={!!searchStore.selectedFilters[option.key]}
					setSelected={(value) =>
						value
							? getSearchStore().selectFilter(option.key, true)
							: getSearchStore().deselectFilter(option.key)
					}
					key={option.key}
					icon="Folder"
				>
					{option.name}
				</SearchOptionItem>
			))}
		</SearchOptionSubMenu>
	);
}

export function TagOptions() {
	const searchStore = useSearchStore();
	const tagsQuery = useLibraryQuery(['tags.list'], { keepPreviousData: true });

	const searchOption = useSearchOption({
		name: 'Tagged',
		icon: CircleDashed,
		options:
			tagsQuery.data?.map((tag) => ({
				key: `tag-${tag.id}`,
				name: tag.name || '',
				icon: tag.color as any
			})) || []
	});

	return (
		<SearchOptionSubMenu name={searchOption.name} icon={searchOption.icon}>
			<FilterInput searchOption={searchOption} />
			<Separator />
			{searchOption.options.map((option) => (
				<SearchOptionItem
					selected={!!searchStore.selectedFilters[option.key]}
					setSelected={(value) =>
						value
							? getSearchStore().selectFilter(option.key, true)
							: getSearchStore().deselectFilter(option.key)
					}
					key={option.key}
					icon={option.icon}
				>
					{option.name}
				</SearchOptionItem>
			))}
		</SearchOptionSubMenu>
	);
}
export function KindOptions() {
	const searchStore = useSearchStore();

	const searchOption = useSearchOption({
		name: 'Kind',
		icon: CircleDashed,
		options:
			Object.keys(ObjectKind)
				.filter((key) => isNaN(Number(key)))
				.map((kind) => ({
					key: `kind-${kind}`,
					name: kind || '',
					icon: kind
				})) || []
	});

	return (
		<SearchOptionSubMenu name={searchOption.name} icon={searchOption.icon}>
			<FilterInput searchOption={searchOption} />
			<Separator />
			{searchOption.options.map((option) => (
				<SearchOptionItem
					selected={!!searchStore.selectedFilters[option.key]}
					setSelected={(value) =>
						value
							? getSearchStore().selectFilter(option.key, true)
							: getSearchStore().deselectFilter(option.key)
					}
					key={option.key}
					icon={option.icon}
				>
					{option.name}
				</SearchOptionItem>
			))}
		</SearchOptionSubMenu>
	);
}
