import { CircleDashed, Folder } from '@phosphor-icons/react';
import { useMemo } from 'react';
import { ObjectKind, useLibraryQuery } from '@sd/client';

import { FilterInput, SearchOptionItem, SearchOptionSubMenu, Separator } from '.';
import { deselectFilter, FilterType, selectFilter, useCreateFilter, useSearchStore } from './store';

type FilterProps = {
	type: FilterType;
};

const FilterComponent: React.FC<FilterProps> = ({ type }) => {
	const locationsQuery = useLibraryQuery(['locations.list'], { keepPreviousData: true });
	const tagsQuery = useLibraryQuery(['tags.list'], { keepPreviousData: true });
	const searchStore = useSearchStore();
	let filters: any[];

	switch (type) {
		case FilterType.Location:
			filters =
				locationsQuery.data?.map((location) => ({
					name: location.name!,
					type,
					value: String(location.id),
					icon: 'Folder'
				})) || [];
			break;

		case FilterType.Tag:
			filters =
				tagsQuery.data?.map((tag) => ({
					name: tag.name!,
					type,
					value: String(tag.id),
					icon: tag.color || 'CircleDashed'
				})) || [];
			break;

		case FilterType.Kind:
			filters =
				Object.keys(ObjectKind)
					.filter((key) => !isNaN(Number(key)) && ObjectKind[Number(key)] !== undefined)
					.map((key) => {
						const kind = ObjectKind[Number(key)];
						return {
							name: kind as string,
							type,
							value: key,
							icon: kind || 'CircleDashed'
						};
					}) || [];
			break;

		default:
			filters = [];
	}

	const createdFilters = useCreateFilter(filters);

	return (
		<SearchOptionSubMenu name={FilterType[type]} icon={CircleDashed}>
			<FilterInput />
			<Separator />
			{createdFilters.map((filter) => (
				<SearchOptionItem
					selected={searchStore.selectedFilters.has(filter.key)}
					setSelected={(value) =>
						value ? selectFilter(filter, true) : deselectFilter(filter)
					}
					key={filter.key}
					icon={filter.icon}
				>
					{filter.name}
				</SearchOptionItem>
			))}
		</SearchOptionSubMenu>
	);
};

export function LocationFilter() {
	return <FilterComponent type={FilterType.Location} />;
}

export function TagFilter() {
	return <FilterComponent type={FilterType.Tag} />;
}

export function KindFilter() {
	return <FilterComponent type={FilterType.Kind} />;
}
