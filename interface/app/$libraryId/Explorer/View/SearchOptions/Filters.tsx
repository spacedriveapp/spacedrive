import { CircleDashed, Cube, Folder, Icon, SelectionSlash, Textbox } from '@phosphor-icons/react';
import { produce } from 'immer';
import { useState } from 'react';
import { InOrNotIn, ObjectKind, SearchFilterArgs, TextMatch, useLibraryQuery } from '@sd/client';
import { Button, Input } from '@sd/ui';

import { SearchOptionItem, SearchOptionSubMenu } from '.';
import {
	AllKeys,
	deselectFilterOption,
	FilterArgs,
	getSearchStore,
	selectFilterOption,
	SetFilter,
	useSearchStore
} from './store';
import { FilterTypeCondition, filterTypeCondition, inOrNotIn, textMatch } from './util';

export interface SearchFilter<
	TConditions extends FilterTypeCondition[keyof FilterTypeCondition] = any
> {
	name: string;
	icon: Icon;
	conditions: TConditions;
}

interface SearchFilterCRUD<
	TConditions extends FilterTypeCondition[keyof FilterTypeCondition] = any,
	T = any
> extends SearchFilter<TConditions> {
	getCondition: (args: T) => keyof TConditions | undefined;
	setCondition: (args: T, condition: keyof TConditions) => void;
	getOptionActive: (args: T, option: FilterArgs) => boolean;
	applyAdd: (args: T, option: FilterArgs) => void;
	applyRemove: (args: T, option: FilterArgs) => T | undefined;
	find: (arg: SearchFilterArgs) => T | undefined;
	create: () => SearchFilterArgs;
}

export interface RenderSearchFilter<
	TConditions extends FilterTypeCondition[keyof FilterTypeCondition] = any,
	T = any
> extends SearchFilterCRUD<TConditions, T> {
	// Render is responsible for fetching the filter options and rendering them
	Render: (props: {
		filter: SearchFilterCRUD<TConditions>;
		options: (FilterArgs & { type: string })[];
	}) => JSX.Element;
	// Apply is responsible for applying the filter to the search args
	useOptions: (props: { search: string }) => FilterArgs[];
}

const FilterOptionList = ({
	filter,
	options
}: {
	filter: SearchFilterCRUD;
	options: FilterArgs[];
}) => {
	const store = useSearchStore();

	return (
		<SearchOptionSubMenu name={filter.name} icon={filter.icon}>
			{options?.map((option) => (
				<SearchOptionItem
					selected={filter.getOptionActive?.(store.filterArgs, option) ?? false}
					setSelected={(value) => {
						getSearchStore().filterArgs = produce(store.filterArgs, (args) => {
							if (!filter.getCondition?.(args))
								filter.setCondition(args, Object.keys(filter.conditions)[0]!);

							if (value) filter.applyAdd(args, option);
							else filter.applyRemove(args, option);
						});

						if (value) selectFilterOption({ ...option, type: filter.name });
						else deselectFilterOption({ ...option, type: filter.name });
					}}
					key={option.value}
					icon={option.icon}
				>
					{option.name}
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
			<Button variant="accent">Apply</Button>
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

function createFilter<TConditions extends FilterTypeCondition[keyof FilterTypeCondition], T>(
	filter: RenderSearchFilter<TConditions, T>
) {
	return filter;
}

function createInOrNotInFilter<T>(
	filter: Omit<
		ReturnType<typeof createFilter<any, InOrNotIn<T>>>,
		| 'conditions'
		| 'getCondition'
		| 'getOptionActive'
		| 'setCondition'
		| 'applyAdd'
		| 'applyRemove'
		| 'create'
	> & {
		create(value: InOrNotIn<T>): SearchFilterArgs;
	}
): ReturnType<typeof createFilter<(typeof filterTypeCondition)['inOrNotIn'], InOrNotIn<T>>> {
	return {
		...filter,
		create: () => {
			return filter.create({ in: [] });
		},
		conditions: filterTypeCondition.inOrNotIn,
		getCondition: (data) => {
			if ('in' in data) return 'in';
			else return 'notIn';
		},
		setCondition: (data, condition) => {
			const contents = 'in' in data ? data.in : data.notIn;

			return condition === 'in' ? { in: contents } : { notIn: contents };
		},
		getOptionActive: (data, option) => {
			if ('in' in data) return data.in.includes(option.value);
			else return data.notIn.includes(option.value);
		},
		applyAdd: (data, option) => {
			if ('in' in data) data.in.push(option.value);
			else data.notIn.push(option.value);

			return data;
		},
		applyRemove: (data, option) => {
			if ('in' in data) {
				data.in = data.in.filter((id) => id !== option.value);

				if (data.in.length === 0) return;
			} else {
				data.notIn = data.notIn.filter((id) => id !== option.value);

				if (data.notIn.length === 0) return;
			}

			return data;
		}
	};
}

export const filterRegistry = [
	createInOrNotInFilter({
		name: 'Location',
		icon: Folder, // Phosphor folder icon
		find: (arg) => {
			if ('filePath' in arg && 'locations' in arg.filePath) return arg.filePath.locations;
		},
		create: (locations) => ({ filePath: { locations } }),
		useOptions: () => {
			const query = useLibraryQuery(['locations.list'], { keepPreviousData: true });

			return (query.data ?? []).map((location) => ({
				name: location.name!,
				value: location.id,
				icon: 'Folder' // Spacedrive folder icon
			}));
		},
		Render: ({ filter, options }) => {
			return <FilterOptionList filter={filter} options={options} />;
		}
	}),
	createInOrNotInFilter({
		name: 'Tags',
		icon: CircleDashed,
		find: (arg) => {
			if ('object' in arg && 'tags' in arg.object) return arg.object.tags;
		},
		create: (tags) => ({ object: { tags } }),
		useOptions: () => {
			const query = useLibraryQuery(['tags.list'], { keepPreviousData: true });

			return (query.data ?? []).map((tag) => ({
				name: tag.name!,
				value: tag.id,
				icon: tag.color || 'CircleDashed'
			}));
		},
		Render: ({ filter, options }) => {
			return <FilterOptionList filter={filter} options={options} />;
		}
	}),
	createInOrNotInFilter({
		name: 'Kind',
		icon: Cube,
		find: (arg) => {
			if ('object' in arg && 'kind' in arg.object) return arg.object.kind;
		},
		create: (kind) => ({ object: { kind } }),
		useOptions: () => {
			return Object.keys(ObjectKind)
				.filter((key) => !isNaN(Number(key)) && ObjectKind[Number(key)] !== undefined)
				.map((key) => {
					const kind = ObjectKind[Number(key)];
					return {
						name: kind as string,
						value: Number(key),
						icon: kind
					};
				});
		},
		Render: ({ filter, options }) => {
			return <FilterOptionList filter={filter} options={options} />;
		}
	}),
	createFilter({
		name: 'Name',
		icon: Textbox,
		conditions: filterTypeCondition.textMatch,
		find: (arg) => {
			if ('filePath' in arg && 'name' in arg.filePath) return arg.filePath.name;
		},
		create: () => ({ filePath: { name: { contains: '' } } }),
		getCondition: (data) => {
			if ('contains' in data) return 'contains';
			else if ('startsWith' in data) return 'startsWith';
			else if ('endsWith' in data) return 'endsWith';
			else if ('equals' in data) return 'equals';
		},
		setCondition: (data, condition) => {
			let value: string;

			if ('contains' in data) value = data.contains;
			else if ('startsWith' in data) value = data.startsWith;
			else if ('endsWith' in data) value = data.endsWith;
			else value = data.equals;

			return {
				[condition]: value
			};
		},
		getOptionActive: (data, option) => {
			if ('contains' in data) return data.contains === option.value;
			else if ('startsWith' in data) return data.startsWith === option.value;
			else if ('endsWith' in data) return data.endsWith === option.value;
			else return data.equals === option.value;
		},
		applyAdd: (data, { value }) => {
			if ('contains' in data) return { contains: value };
			else if ('startsWith' in data) return { startsWith: value };
			else if ('endsWith' in data) return { endsWith: value };
			else if ('equals' in data) return { equals: value };
		},
		applyRemove: () => undefined,
		useOptions: ({ search }) => {
			return [
				{
					name: search,
					value: search,
					icon: 'Textbox'
				}
			];
		},
		Render: ({ filter }) => {
			return <FilterOptionText filter={filter} />;
		}
	}),
	createInOrNotInFilter({
		name: 'Extension',
		icon: Textbox,
		find: (arg) => {
			if ('filePath' in arg && 'extension' in arg.filePath) return arg.filePath.extension;
		},
		create: (extension) => ({ filePath: { extension } }),
		useOptions: ({ search }) => {
			return [
				{
					name: search,
					value: search,
					icon: 'Textbox'
				}
			];
		},
		Render: ({ filter }) => {
			return <FilterOptionText filter={filter} />;
		}
	})
	// createFilter({
	// 	name: 'Hidden',
	// 	icon: SelectionSlash,
	// 	conditions: filterTypeCondition.trueOrFalse,
	// 	setCondition: (args, condition: 'true' | 'false') => {
	// 		const filePath = (args.filePath ??= {});

	// 		filePath.hidden = condition === 'true';
	// 	},
	// 	applyAdd: () => {},
	// 	applyRemove: (args) => {
	// 		delete args.filePath?.hidden;
	// 	},
	// 	useOptions: () => {
	// 		return [
	// 			{
	// 				name: 'Hidden',
	// 				value: true,
	// 				icon: 'SelectionSlash' // Spacedrive folder icon
	// 			}
	// 		];
	// 	},
	// 	Render: ({ filter }) => {
	// 		return <FilterOptionBoolean filter={filter} />;
	// 	},
	// 	apply(filter, args) {
	// 		(args.filePath ??= {}).hidden = filter.condition;
	// 	}
	// }),
	// createFilter({
	// 	name: 'WithDescendants',
	// 	icon: SelectionSlash,
	// 	conditions: filterTypeCondition.trueOrFalse,
	// 	setCondition: (args, condition: 'true' | 'false') => {
	// 		const filePath = (args.filePath ??= {});

	// 		filePath.withDescendants = condition === 'true';
	// 	},
	// 	applyAdd: () => {},
	// 	applyRemove: (args) => {
	// 		delete args.filePath?.withDescendants;
	// 	},
	// 	useOptions: () => {
	// 		return [
	// 			{
	// 				name: 'With Descendants',
	// 				value: true,
	// 				icon: 'SelectionSlash' // Spacedrive folder icon
	// 			}
	// 		];
	// 	},
	// 	Render: ({ filter }) => {
	// 		return <FilterOptionBoolean filter={filter} />;
	// 	},
	// 	apply(filter, args) {
	// 		(args.filePath ??= {}).withDescendants = filter.condition;
	// 	}
	// })
] as const satisfies ReadonlyArray<RenderSearchFilter<any>>;

export type FilterType = (typeof filterRegistry)[number]['name'];
