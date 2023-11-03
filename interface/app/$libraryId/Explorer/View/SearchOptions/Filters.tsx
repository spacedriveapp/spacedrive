import { CircleDashed, Cube, Folder, Icon, SelectionSlash, Textbox } from '@phosphor-icons/react';
import { produce } from 'immer';
import { useState } from 'react';
import { ref, snapshot } from 'valtio';
import { InOrNotIn, ObjectKind, SearchFilterArgs, TextMatch, useLibraryQuery } from '@sd/client';
import { Button, Input } from '@sd/ui';

import { SearchOptionItem, SearchOptionSubMenu } from '.';
import {
	AllKeys,
	deselectFilterOption,
	FilterOption,
	getKey,
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
	getOptionActive: (args: T, option: FilterOption) => boolean;
	getActiveOptions: (args: T, allOptions: FilterOption[]) => FilterOption[];
	applyAdd: (args: T, option: FilterOption) => void;
	applyRemove: (args: T, option: FilterOption) => T | undefined;
	argsToOptions: (args: T) => FilterOption[];
	find: (arg: SearchFilterArgs) => T | undefined;
	create: (data: any) => SearchFilterArgs;
}

export interface RenderSearchFilter<
	TConditions extends FilterTypeCondition[keyof FilterTypeCondition] = any,
	T = any
> extends SearchFilterCRUD<TConditions, T> {
	// Render is responsible for fetching the filter options and rendering them
	Render: (props: {
		filter: SearchFilterCRUD<TConditions>;
		options: (FilterOption & { type: string })[];
	}) => JSX.Element;
	// Apply is responsible for applying the filter to the search args
	useOptions: (props: { search: string }) => FilterOption[];
}

const FilterOptionList = ({
	filter,
	options
}: {
	filter: SearchFilterCRUD;
	options: FilterOption[];
}) => {
	const store = useSearchStore();

	const arg = store.filterArgs.find(filter.find);
	const specificArg = arg ? filter.find(arg) : undefined;

	return (
		<SearchOptionSubMenu name={filter.name} icon={filter.icon}>
			{options?.map((option) => (
				<SearchOptionItem
					selected={
						(specificArg && filter.getOptionActive?.(specificArg, option)) ?? false
					}
					setSelected={(value) => {
						const searchStore = getSearchStore();

						searchStore.filterArgs = ref(
							produce(searchStore.filterArgs, (args) => {
								const key = getKey({
									type: filter.name,
									name: option.name,
									value: option.value
								});

								if (searchStore.fixedFilterKeys.has(key)) return;

								let rawArg = args.find((arg) => filter.find(arg));

								if (!rawArg) {
									rawArg = filter.create();
									args.push(rawArg);
								}

								const rawArgIndex = args.findIndex((arg) => filter.find(arg))!;

								const arg = filter.find(rawArg)!;

								if (!filter.getCondition?.(arg))
									filter.setCondition(arg, Object.keys(filter.conditions)[0]!);

								if (value) filter.applyAdd(arg, option);
								else filter.applyRemove(arg, option);

								if (!filter.getActiveOptions?.(arg, options).length) {
									args.splice(rawArgIndex);
								}
							})
						);
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

const FilterOptionText = ({ filter }: { filter: SearchFilterCRUD }) => {
	const [value, setValue] = useState('');
	const store = useSearchStore();

	return (
		<SearchOptionSubMenu name={filter.name} icon={filter.icon}>
			<Input value={value} onChange={(e) => setValue(e.target.value)} />
			<Button
				variant="accent"
				onClick={() => {
					const searchStore = getSearchStore();

					searchStore.filterArgs = ref(
						produce(searchStore.filterArgs, (args) => {
							const key = getKey({
								type: filter.name,
								name: value,
								value
							});

							if (searchStore.fixedFilterKeys.has(key)) return;

							const arg = filter.create(value);
							args.push(arg);

							filter.applyAdd(filter.find(arg)!, { name: value, value });
						})
					);

					console.log(snapshot(searchStore).filterArgs);
				}}
			>
				Apply
			</Button>
		</SearchOptionSubMenu>
	);
};

const FilterOptionBoolean = ({ filter }: { filter: SearchFilterCRUD }) => {
	const { filterArgs } = useSearchStore();

	return (
		<SearchOptionItem
			icon={filter.icon}
			selected={filterArgs.find((a) => filter.find(a) !== undefined) !== undefined}
			setSelected={() => {
				const searchStore = getSearchStore();

				searchStore.filterArgs = ref(
					produce(filterArgs, (args) => {
						const key = getKey({
							type: filter.name,
							name: filter.name,
							value: true
						});

						if (searchStore.fixedFilterKeys.has(key)) return;

						const index = args.findIndex((f) => filter.find(f) !== undefined);

						if (index !== -1) {
							args.splice(index);
						} else {
							const arg = filter.create();
							args.push(arg);

							filter.applyAdd(arg, { name: filter.name, value: true });
						}
					})
				);
			}}
		>
			{filter.name}
		</SearchOptionItem>
	);
};

function createFilter<TConditions extends FilterTypeCondition[keyof FilterTypeCondition], T>(
	filter: RenderSearchFilter<TConditions, T>
) {
	return filter;
}

function createInOrNotInFilter<T extends string | number>(
	filter: Omit<
		ReturnType<typeof createFilter<any, InOrNotIn<T>>>,
		| 'conditions'
		| 'getCondition'
		| 'getOptionActive'
		| 'getActiveOptions'
		| 'argsToOptions'
		| 'setCondition'
		| 'applyAdd'
		| 'applyRemove'
		| 'create'
	> & {
		create(value: InOrNotIn<T>): SearchFilterArgs;
		argsToOptions(values: T[]): FilterOption[];
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
		getActiveOptions: (data, options) => {
			let value: T[];

			if ('in' in data) value = data.in;
			else value = data.notIn;

			return value.map((v) => options.find((o) => o.value === v)!).filter(Boolean);
		},
		argsToOptions: (data) => {
			let values: T[];

			if ('in' in data) values = data.in;
			else values = data.notIn;

			return filter.argsToOptions(values);
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

function createTextMatchFilter(
	filter: Omit<
		ReturnType<typeof createFilter<any, TextMatch>>,
		| 'conditions'
		| 'getCondition'
		| 'getOptionActive'
		| 'getActiveOptions'
		| 'argsToOptions'
		| 'setCondition'
		| 'applyAdd'
		| 'applyRemove'
		| 'create'
	> & {
		create(value: TextMatch): SearchFilterArgs;
	}
): ReturnType<typeof createFilter<(typeof filterTypeCondition)['textMatch'], TextMatch>> {
	return {
		...filter,
		conditions: filterTypeCondition.textMatch,
		create: (contains) => {
			return filter.create({ contains });
		},
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
		getActiveOptions: (data) => {
			let value: string;

			if ('contains' in data) value = data.contains;
			else if ('startsWith' in data) value = data.startsWith;
			else if ('endsWith' in data) value = data.endsWith;
			else value = data.equals;

			return [
				{
					type: filter.name,
					name: value,
					value
				}
			];
		},
		argsToOptions: (data) => {
			let value: string;

			if ('contains' in data) value = data.contains;
			else if ('startsWith' in data) value = data.startsWith;
			else if ('endsWith' in data) value = data.endsWith;
			else value = data.equals;

			return [
				{
					type: filter.name,
					name: value,
					value
				}
			];
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
		applyRemove: () => undefined
	};
}

function createBooleanFilter(
	filter: Omit<
		ReturnType<typeof createFilter<any, boolean>>,
		| 'conditions'
		| 'getCondition'
		| 'getOptionActive'
		| 'getActiveOptions'
		| 'argsToOptions'
		| 'setCondition'
		| 'applyAdd'
		| 'applyRemove'
		| 'create'
	> & {
		create(value: boolean): SearchFilterArgs;
	}
): ReturnType<typeof createFilter<(typeof filterTypeCondition)['trueOrFalse'], boolean>> {
	return {
		...filter,
		conditions: filterTypeCondition.trueOrFalse,
		create: () => {
			return filter.create(true);
		},
		getCondition: (data) => {
			return data ? 'true' : 'false';
		},
		setCondition: (_, condition) => {
			return condition === 'true';
		},
		argsToOptions: (value) => {
			const option = getSearchStore()
				.filterOptions.get(filter.name)
				?.find((o) => o.value === value);

			if (!option) return [];

			return [
				{
					type: filter.name,
					name: option.name,
					value
				}
			];
		},
		getActiveOptions: (data, options) => {
			return options.filter((o) => o.value === data);
		},
		getOptionActive: (data, option) => {
			return option.value === data;
		},
		applyAdd: (_, { value }) => {
			return value;
		},
		applyRemove: () => undefined
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
		argsToOptions(values) {
			return values
				.map((value) => {
					const option = getSearchStore()
						.filterOptions.get(this.name)
						?.find((o) => o.value === value);

					if (!option) return;

					return {
						type: this.name,
						name: value,
						value
					};
				})
				.filter(Boolean) as any;
		},
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
		argsToOptions(values) {
			return values
				.map((value) => {
					const option = getSearchStore()
						.filterOptions.get(this.name)
						?.find((o) => o.value === value);

					if (!option) return;

					return {
						type: this.name,
						name: value,
						value
					};
				})
				.filter(Boolean) as any;
		},
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
		argsToOptions(values) {
			return values
				.map((value) => {
					const option = getSearchStore()
						.filterOptions.get(this.name)
						?.find((o) => o.value === value);

					if (!option) return;

					return {
						type: this.name,
						name: value,
						value
					};
				})
				.filter(Boolean) as any;
		},
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
	createTextMatchFilter({
		name: 'Name',
		icon: Textbox,
		find: (arg) => {
			if ('filePath' in arg && 'name' in arg.filePath) return arg.filePath.name;
		},
		create: (name) => ({ filePath: { name } }),
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
		argsToOptions(values) {
			return values.map((value) => ({
				type: this.name,
				name: value,
				value
			}));
		},
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
	createBooleanFilter({
		name: 'Hidden',
		icon: SelectionSlash,
		find: (arg) => {
			if ('filePath' in arg && 'hidden' in arg.filePath) return arg.filePath.hidden;
		},
		create: (hidden) => ({ filePath: { hidden } }),
		useOptions: () => {
			return [
				{
					name: 'Hidden',
					value: true,
					icon: 'SelectionSlash' // Spacedrive folder icon
				}
			];
		},
		Render: ({ filter }) => {
			return <FilterOptionBoolean filter={filter} />;
		}
	})
	// idk how to handle this rn since include_descendants is part of 'path' now
	//
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
