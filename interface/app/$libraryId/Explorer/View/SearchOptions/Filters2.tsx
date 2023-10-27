import { CircleDashed, Cube, Folder, Icon, Textbox } from '@phosphor-icons/react';
import { ObjectKind, useLibraryQuery } from '@sd/client';

import { SearchOptionItem, SearchOptionSubMenu } from '.';
import { deselectFilter, FilterArgs, selectFilter, useSearchStore } from './store';

export enum FilterType {
	Location,
	Tag,
	Kind,
	Category,
	Size,
	Name,
	Extension,
	CreatedAt,
	WithDescendants,
	Hidden
}

export enum FilterMethod {
	TrueOrFalse, // true | false
	InOrNotIn, // { in: T[] } | { not_in: T[] }
	MaybeNot, // { not: T }
	TextMatch, // { matches: T } | { contains: T }
	OptionalRange // { from: T, to: T }
}

export interface SearchFilter {
	name: string;
	icon: Icon;
	method: FilterMethod;
	type: FilterType;
}
export interface RenderSearchFilter extends SearchFilter {
	Render: (filter: SearchFilter) => JSX.Element;
}

export type FilterRegistry = Record<FilterType, RenderSearchFilter>;

// @ts-expect-error
export const filterRegistry: FilterRegistry = {
	[FilterType.Location]: {
		name: 'Location',
		icon: Folder,
		method: FilterMethod.InOrNotIn,
		type: FilterType.Location,
		Render: (filter) => {
			const query = useLibraryQuery(['locations.list']);
			return (
				<FilterOptionList
					filter={filter}
					options={query.data?.map((location) => ({
						name: location.name!,
						value: String(location.id),
						icon: 'Folder'
					}))}
				/>
			);
		}
	},
	[FilterType.Tag]: {
		name: 'Tags',
		icon: CircleDashed,
		method: FilterMethod.InOrNotIn,
		type: FilterType.Tag,
		Render: (filter) => {
			const query = useLibraryQuery(['tags.list']);
			return (
				<FilterOptionList
					filter={filter}
					options={query.data?.map((tag) => ({
						name: tag.name!,
						value: String(tag.id),
						icon: tag.color || 'CircleDashed'
					}))}
				/>
			);
		}
	},
	[FilterType.Kind]: {
		name: 'Kind',
		icon: Cube,
		method: FilterMethod.InOrNotIn,
		type: FilterType.Kind,
		Render: (filter) => {
			return (
				<FilterOptionList
					filter={filter}
					options={Object.keys(ObjectKind)
						.filter(
							(key) => !isNaN(Number(key)) && ObjectKind[Number(key)] !== undefined
						)
						.map((key) => {
							const kind = ObjectKind[Number(key)];
							return {
								name: kind as string,
								value: key,
								icon: 'Cube'
							};
						})}
				/>
			);
		}
	},
	[FilterType.Name]: {
		name: 'Name',
		icon: Textbox,
		method: FilterMethod.TextMatch,
		type: FilterType.Name,
		Render: (filter) => {
			return <FilterOptionText filter={filter} />;
		}
	},
	[FilterType.Extension]: {
		name: 'Extension',
		icon: Textbox,
		method: FilterMethod.InOrNotIn,
		type: FilterType.Extension,
		Render: (filter) => {
			return <FilterOptionList filter={filter} />;
		}
	}
};

const FilterOptionList: React.FC<{ filter: SearchFilter; options?: FilterArgs[] }> = (props) => {
	const store = useSearchStore();
	const options = props.options?.map((filter) => ({ ...filter, type: props.filter.type }));
	return (
		<SearchOptionSubMenu name={props.filter.name} icon={props.filter.icon}>
			{options?.map((filter) => (
				<SearchOptionItem
					selected={store.selectedFilters.has(filter.value)}
					setSelected={(value) => (value ? selectFilter(filter) : deselectFilter(filter))}
					key={filter.value}
					icon={filter.icon}
				>
					{filter.name}
				</SearchOptionItem>
			))}
		</SearchOptionSubMenu>
	);
};

// a sub menu with a text input and accent button to add the text as a filter
// const FilterOptionText: React.FC<{ filter: SearchFilter }> = (props) => {

// }
