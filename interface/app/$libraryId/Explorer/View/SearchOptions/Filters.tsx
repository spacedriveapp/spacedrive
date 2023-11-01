import { CircleDashed, Cube, Folder, Icon, SelectionSlash, Textbox } from '@phosphor-icons/react';
import { useState } from 'react';
import { ObjectKind, SearchFilterArgs, useLibraryQuery } from '@sd/client';
import { Input } from '@sd/ui';

import { SearchOptionItem, SearchOptionSubMenu } from '.';
import {
	deselectFilter,
	FilterArgs,
	selectFilter,
	SetFilter,
	useRegisterSearchFilterOptions,
	useSearchStore
} from './store';
import { inOrNotIn, textMatch } from './util';

export interface SearchFilter {
	name: string;
	icon: Icon;
}

export interface RenderSearchFilter extends SearchFilter {
	// Render is responsible for fetching the filter options and rendering them
	Render: (props: { filter: SearchFilter }) => JSX.Element;
	// Apply is responsible for applying the filter to the search args
	apply: (filter: SetFilter, args: SearchFilterArgs) => void;
}

const FilterOptionList: React.FC<{ filter: SearchFilter; options?: FilterArgs[] }> = (props) => {
	const store = useSearchStore();
	const options = useRegisterSearchFilterOptions(props.filter.name as FilterType, props.options);

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

const FilterOptionText: React.FC<{ filter: SearchFilter }> = (props) => {
	const [value, setValue] = useState('');
	return (
		<SearchOptionSubMenu name={props.filter.name} icon={props.filter.icon}>
			<Input />
		</SearchOptionSubMenu>
	);
};

const FilterOptionBoolean: React.FC<{ filter: SearchFilter }> = (props) => {
	return (
		<SearchOptionItem icon={props.filter.icon} selected={false} setSelected={() => {}}>
			{props.filter.name}
		</SearchOptionItem>
	);
};

export const filterTypeRegistry = [
	{
		name: 'Location',
		icon: Folder, // Phosphor folder icon
		Render: ({ filter }) => {
			const query = useLibraryQuery(['locations.list'], { keepPreviousData: true });
			return (
				<FilterOptionList
					filter={filter}
					options={query.data?.map((location) => ({
						name: location.name!,
						value: location.id,
						icon: 'Folder', // Spacedrive folder icon
						type: filter.name
					}))}
				/>
			);
		},
		apply: (filter, args) =>
			((args.filePath ??= {}).locations = inOrNotIn(
				args.filePath?.locations,
				filter.value,
				filter.condition
			))
	},
	{
		name: 'Tags',
		icon: CircleDashed,
		Render: ({ filter }) => {
			const query = useLibraryQuery(['tags.list'], { keepPreviousData: true });
			return (
				<FilterOptionList
					filter={filter}
					options={query.data?.map((tag) => ({
						name: tag.name!,
						value: String(tag.id),
						icon: tag.color || 'CircleDashed',
						type: filter.name
					}))}
				/>
			);
		},
		apply: (filter, args) => {
			(args.object ??= {}).tags = inOrNotIn(
				args.object?.tags,
				filter.value,
				filter.condition
			);
		}
	},
	{
		name: 'Kind',
		icon: Cube,
		Render: ({ filter }) => {
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
								icon: 'Cube',
								type: filter.name
							};
						})}
				/>
			);
		},
		apply: (filter, args) => {
			(args.object ??= {}).kind = inOrNotIn(
				args.object?.kind,
				filter.value,
				filter.condition
			);
		}
	},
	{
		name: 'Name',
		icon: Textbox,
		Render: ({ filter }) => {
			return <FilterOptionText filter={filter} />;
		},
		apply: (filter, args) => {
			(args.filePath ??= {}).name = textMatch('contains')(filter.value);
		}
	},
	{
		name: 'Extension',
		icon: Textbox,
		Render: ({ filter }) => {
			return <FilterOptionText filter={filter} />;
		},
		apply: (filter, currentArgs) => ({
			filePath: {
				extension: inOrNotIn(
					currentArgs.filePath?.extension,
					filter.value,
					filter.condition
				)
			}
		})
	},
	{
		name: 'Hidden',
		icon: SelectionSlash,
		Render: ({ filter }) => {
			return <FilterOptionBoolean filter={filter} />;
		},
		apply(filter, args) {
			(args.filePath ??= {}).hidden = filter.condition;
		}
	},
	{
		name: 'WithDescendants',
		icon: SelectionSlash,
		Render: ({ filter }) => {
			return <FilterOptionBoolean filter={filter} />;
		},
		apply(filter, args) {
			(args.filePath ??= {}).withDescendants = filter.condition;
		}
	}
] as const satisfies ReadonlyArray<RenderSearchFilter>;

export type FilterType = (typeof filterTypeRegistry)[number]['name'];
