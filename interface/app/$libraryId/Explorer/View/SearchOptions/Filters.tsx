import {
	CircleDashed,
	Clock,
	Cube,
	FileDoc,
	Files,
	Folder,
	SelectionSlash
} from '@phosphor-icons/react';
import { ObjectKind, useLibraryQuery } from '@sd/client';

import { SearchOptionItem, SearchOptionSubMenu } from '.';
import { FilterTypeMeta, selectFilter, useSearchFilter, useSearchStore } from './store';

export enum FilterType {
	Location,
	Tag,
	Kind,
	Category,
	Size,
	Name,
	Extension,
	CreatedAt,
	// ModifiedAt,
	// LastOpenedAt,
	// TakenAt,
	Hidden
	// FileContents,
	// Album,
	// Device,
	// Key,
	// Contact,
}

export const LocationsFilter: React.FC = () => {
	const store = useSearchStore();
	const query = useLibraryQuery(['locations.list']);

	const filter = useSearchFilter(
		FilterType.Location,
		query.data?.map((location) => ({
			name: location.name!,
			value: String(location.id),
			icon: 'Folder'
		}))
	);

	return (
		<SearchOptionSubMenu
			name={filterMeta[FilterType.Location].name}
			icon={filterMeta[FilterType.Location].icon}
		>
			{filter.map((filter) => (
				<SearchOptionItem
					selected={store.selectedFilters.has(filter.key)}
					setSelected={(value) => selectFilter(filter, value, true)}
					key={filter.key}
					icon={filter.icon}
				>
					{filter.name}
				</SearchOptionItem>
			))}
		</SearchOptionSubMenu>
	);
};

export const TagsFilter: React.FC = () => {
	const store = useSearchStore();
	const query = useLibraryQuery(['tags.list']);

	const filter = useSearchFilter(
		FilterType.Tag,
		query.data?.map((tag) => ({
			name: tag.name!,
			value: String(tag.id),
			icon: tag.color || 'CircleDashed'
		}))
	);

	return (
		<SearchOptionSubMenu
			name={filterMeta[FilterType.Tag].name}
			icon={filterMeta[FilterType.Tag].icon}
		>
			{filter.map((filter) => (
				<SearchOptionItem
					selected={store.selectedFilters.has(filter.key)}
					setSelected={(value) => selectFilter(filter, value, true)}
					key={filter.key}
					icon={filter.icon}
				>
					{filter.name}
				</SearchOptionItem>
			))}
		</SearchOptionSubMenu>
	);
};

export const KindsFilter: React.FC = () => {
	const store = useSearchStore();

	const filter = useSearchFilter(
		FilterType.Kind,
		Object.keys(ObjectKind)
			.filter((key) => !isNaN(Number(key)) && ObjectKind[Number(key)] !== undefined)
			.map((key) => {
				const kind = ObjectKind[Number(key)];
				return {
					name: kind as string,
					value: key,
					icon: kind || 'CircleDashed'
				};
			})
	);

	return (
		<SearchOptionSubMenu
			name={filterMeta[FilterType.Kind].name}
			icon={filterMeta[FilterType.Kind].icon}
		>
			{filter.map((filter) => (
				<SearchOptionItem
					selected={store.selectedFilters.has(filter.key)}
					setSelected={(value) => selectFilter(filter, value, true)}
					key={filter.key}
					icon={filter.icon}
				>
					{filter.name}
				</SearchOptionItem>
			))}
		</SearchOptionSubMenu>
	);
};

// const tagsQuery = useLibraryQuery(['tags.list'], { keepPreviousData: true });

// tagsQuery.data?.map((tag) => ({
// 	name: tag.name!,
// 	type,
// 	value: String(tag.id),
// 	icon: tag.color || 'CircleDashed'
// }))

export const filterMeta: Record<FilterType, FilterTypeMeta> = {
	[FilterType.Location]: {
		name: 'Location',
		icon: Folder,
		wording: {
			singular: 'is',
			plural: 'is any of',
			singularNot: 'is not',
			pluralNot: 'is not any of'
		}
	},
	[FilterType.Tag]: {
		name: 'Tags',
		icon: CircleDashed,
		wording: {
			singular: 'has',
			plural: 'has any of',
			singularNot: 'does not have',
			pluralNot: 'does not have any of'
		}
	},
	[FilterType.Kind]: {
		name: 'Kind',
		icon: Files,
		wording: {
			singular: 'is',
			plural: 'is any of',
			singularNot: 'is not',
			pluralNot: 'is not any of'
		}
	},
	[FilterType.Category]: {
		name: 'Category',
		icon: CircleDashed,
		wording: {
			singular: 'is',
			plural: 'is any of',
			singularNot: 'is not',
			pluralNot: 'is not any of'
		}
	},
	[FilterType.CreatedAt]: {
		name: 'Created At',
		icon: Clock,
		wording: {
			singular: 'is',
			plural: 'is between',
			singularNot: 'is not',
			pluralNot: 'is not between'
		}
	},
	[FilterType.Hidden]: {
		name: 'Hidden',
		icon: SelectionSlash,
		wording: {
			singular: 'is',
			singularNot: 'is not'
		}
	},
	[FilterType.Size]: {
		name: 'Size',
		icon: Cube,
		wording: {
			singular: 'is',
			singularNot: 'is not'
		}
	},
	[FilterType.Name]: {
		name: 'Name',
		icon: CircleDashed,
		wording: {
			singular: 'is',
			singularNot: 'is not'
		}
	},
	[FilterType.Extension]: {
		name: 'Extension',
		icon: FileDoc,
		wording: {
			singular: 'is',
			singularNot: 'is not'
		}
	}
};

// type FilterProps = {
// 	type: FilterType;
// };
