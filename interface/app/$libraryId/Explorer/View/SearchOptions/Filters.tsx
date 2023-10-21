import { CircleDashed, Folder, Icon, Link, MagnifyingGlass } from '@phosphor-icons/react';
import { useMemo } from 'react';
import { ObjectKind, useLibraryQuery } from '@sd/client';

import { FilterInput, SearchOptionItem, SearchOptionSubMenu, Separator } from '.';
import { deselectFilter, FilterType, selectFilter, useCreateFilter, useSearchStore } from './store';

export const searchFilterTypeMeta: Record<
	FilterType,
	{ name: string; icon: Icon; wording: { singular: string; plural?: string } }
> = {
	[FilterType.Location]: {
		name: 'Location',
		icon: Folder,
		wording: { singular: 'is', plural: 'is any of' }
	},
	[FilterType.Tag]: {
		name: 'Tags',
		icon: CircleDashed,
		wording: { singular: 'has', plural: 'has any of' }
	},
	[FilterType.Kind]: {
		name: 'Kind',
		icon: CircleDashed,
		wording: { singular: 'is', plural: 'is any of' }
	},
	[FilterType.Category]: {
		name: 'Category',
		icon: CircleDashed,
		wording: { singular: 'is', plural: 'is any of' }
	},
	[FilterType.CreatedAt]: {
		name: 'Created At',
		icon: CircleDashed,
		wording: { singular: 'is', plural: 'is between' }
	},
	[FilterType.Hidden]: {
		name: 'Hidden',
		icon: CircleDashed,
		wording: { singular: 'is' }
	}
};

type FilterProps = {
	type: FilterType;
};

export const FilterComponent: React.FC<FilterProps> = ({ type }) => {
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
		<SearchOptionSubMenu
			name={searchFilterTypeMeta[type].name}
			icon={searchFilterTypeMeta[type].icon}
		>
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
