import { CircleDashed, Cube, Folder, Icon, SelectionSlash, Textbox } from '@phosphor-icons/react';
import { useCallback, useState } from 'react';
import { InOrNotIn, ObjectKind, SearchFilterArgs, TextMatch, useLibraryQuery } from '@sd/client';
import { Button, Input } from '@sd/ui';

import { SearchOptionItem, SearchOptionSubMenu } from '.';
import { useSearchContext } from './Context';
import { AllKeys, FilterOption, getKey, updateFilterArgs, useSearchStore } from './store';
import { FilterTypeCondition, filterTypeCondition } from './util';

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
	getCondition: (args: T) => AllKeys<TConditions>;
	setCondition: (args: T, condition: keyof TConditions) => void;
	applyAdd: (args: T, option: FilterOption) => void;
	applyRemove: (args: T, option: FilterOption) => T | undefined;
	argsToOptions: (args: T, options: Map<string, FilterOption[]>) => FilterOption[];
	extract: (arg: SearchFilterArgs) => T | undefined;
	create: (data: any) => SearchFilterArgs;
	merge: (left: T, right: T) => T;
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

export function useToggleOptionSelected() {
	const { fixedArgsKeys } = useSearchContext();

	return useCallback(
		({
			filter,
			option,
			select
		}: {
			filter: (typeof filterRegistry)[number];
			option: FilterOption;
			select: boolean;
		}) =>
			updateFilterArgs((args) => {
				const key = getKey({ ...option, type: filter.name });

				if (fixedArgsKeys?.has(key)) return args;

				const rawArg = args.find((arg) => filter.extract(arg));

				if (!rawArg) {
					const arg = filter.create(option.value);
					args.push(arg);
				} else {
					const rawArgIndex = args.findIndex((arg) => filter.extract(arg))!;

					const arg = filter.extract(rawArg)!;

					if (select) {
						if (rawArg) filter.applyAdd(arg, option);
					} else {
						if (!filter.applyRemove(arg, option)) args.splice(rawArgIndex, 1);
					}
				}

				return args;
			}),
		[fixedArgsKeys]
	);
}

const FilterOptionList = ({ filter, options }: { filter: FilterType; options: FilterOption[] }) => {
	const store = useSearchStore();
	const { fixedArgsKeys } = useSearchContext();

	const toggleOptionSelected = useToggleOptionSelected();

	return (
		<SearchOptionSubMenu name={filter.name} icon={filter.icon}>
			{options?.map((option) => {
				const optionKey = getKey({
					...option,
					type: filter.name
				});

				return (
					<SearchOptionItem
						selected={
							store.filterArgsKeys.has(optionKey) || fixedArgsKeys?.has(optionKey)
						}
						setSelected={(value) =>
							toggleOptionSelected({
								filter,
								option,
								select: value
							})
						}
						key={option.value}
						icon={option.icon}
					>
						{option.name}
					</SearchOptionItem>
				);
			})}
		</SearchOptionSubMenu>
	);
};

const FilterOptionText = ({ filter }: { filter: SearchFilterCRUD }) => {
	const [value, setValue] = useState('');

	const { fixedArgsKeys } = useSearchContext();

	return (
		<SearchOptionSubMenu name={filter.name} icon={filter.icon}>
			<Input value={value} onChange={(e) => setValue(e.target.value)} />
			<Button
				variant="accent"
				onClick={() => {
					updateFilterArgs((args) => {
						const key = getKey({
							type: filter.name,
							name: value,
							value
						});

						if (fixedArgsKeys?.has(key)) return args;

						const arg = filter.create(value);
						args.push(arg);

						return args;
					});
				}}
			>
				Apply
			</Button>
		</SearchOptionSubMenu>
	);
};

const FilterOptionBoolean = ({ filter }: { filter: SearchFilterCRUD }) => {
	const { filterArgsKeys } = useSearchStore();

	const { fixedArgsKeys } = useSearchContext();

	const key = getKey({
		type: filter.name,
		name: filter.name,
		value: true
	});

	return (
		<SearchOptionItem
			icon={filter.icon}
			selected={fixedArgsKeys?.has(key) || filterArgsKeys.has(key)}
			setSelected={() => {
				updateFilterArgs((args) => {
					if (fixedArgsKeys?.has(key)) return args;

					const index = args.findIndex((f) => filter.extract(f) !== undefined);

					if (index !== -1) {
						args.splice(index, 1);
					} else {
						const arg = filter.create(true);
						args.push(arg);
					}

					return args;
				});
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
		| 'argsToOptions'
		| 'setCondition'
		| 'applyAdd'
		| 'applyRemove'
		| 'create'
		| 'merge'
	> & {
		create(value: InOrNotIn<T>): SearchFilterArgs;
		argsToOptions(values: T[], options: Map<string, FilterOption[]>): FilterOption[];
	}
): ReturnType<typeof createFilter<(typeof filterTypeCondition)['inOrNotIn'], InOrNotIn<T>>> {
	return {
		...filter,
		create: (data) => {
			if (typeof data === 'number' || typeof data === 'string')
				return filter.create({
					in: [data as any]
				});
			else if (data) return filter.create(data);
			else return filter.create({ in: [] });
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
		argsToOptions: (data, options) => {
			let values: T[];

			if ('in' in data) values = data.in;
			else values = data.notIn;

			return filter.argsToOptions(values, options);
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
		},
		merge: (left, right) => {
			if ('in' in left && 'in' in right) {
				return {
					in: [...new Set([...left.in, ...right.in])]
				};
			} else if ('notIn' in left && 'notIn' in right) {
				return {
					notIn: [...new Set([...left.notIn, ...right.notIn])]
				};
			}

			throw new Error('Cannot merge InOrNotIns with different conditions');
		}
	};
}

function createTextMatchFilter(
	filter: Omit<
		ReturnType<typeof createFilter<any, TextMatch>>,
		| 'conditions'
		| 'getCondition'
		| 'argsToOptions'
		| 'setCondition'
		| 'applyAdd'
		| 'applyRemove'
		| 'create'
		| 'merge'
	> & {
		create(value: TextMatch): SearchFilterArgs;
	}
): ReturnType<typeof createFilter<(typeof filterTypeCondition)['textMatch'], TextMatch>> {
	return {
		...filter,
		conditions: filterTypeCondition.textMatch,
		create: (contains) => filter.create({ contains }),
		getCondition: (data) => {
			if ('contains' in data) return 'contains';
			else if ('startsWith' in data) return 'startsWith';
			else if ('endsWith' in data) return 'endsWith';
			else return 'equals';
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
		applyAdd: (data, { value }) => {
			if ('contains' in data) return { contains: value };
			else if ('startsWith' in data) return { startsWith: value };
			else if ('endsWith' in data) return { endsWith: value };
			else if ('equals' in data) return { equals: value };
		},
		applyRemove: () => undefined,
		merge: (left) => left
	};
}

function createBooleanFilter(
	filter: Omit<
		ReturnType<typeof createFilter<any, boolean>>,
		| 'conditions'
		| 'getCondition'
		| 'argsToOptions'
		| 'setCondition'
		| 'applyAdd'
		| 'applyRemove'
		| 'create'
		| 'merge'
	> & {
		create(value: boolean): SearchFilterArgs;
	}
): ReturnType<typeof createFilter<(typeof filterTypeCondition)['trueOrFalse'], boolean>> {
	return {
		...filter,
		conditions: filterTypeCondition.trueOrFalse,
		create: () => filter.create(true),
		getCondition: (data) => (data ? 'true' : 'false'),
		setCondition: (_, condition) => condition === 'true',
		argsToOptions: (value) => {
			if (!value) return [];

			return [
				{
					type: filter.name,
					name: filter.name,
					value
				}
			];
		},
		applyAdd: (_, { value }) => value,
		applyRemove: () => undefined,
		merge: (left) => left
	};
}

export const filterRegistry = [
	createInOrNotInFilter({
		name: 'Location',
		icon: Folder, // Phosphor folder icon
		extract: (arg) => {
			if ('filePath' in arg && 'locations' in arg.filePath) return arg.filePath.locations;
		},
		create: (locations) => ({ filePath: { locations } }),
		argsToOptions(values, options) {
			return values
				.map((value) => {
					const option = options.get(this.name)?.find((o) => o.value === value);

					if (!option) return;

					return {
						...option,
						type: this.name
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
		Render: ({ filter, options }) => <FilterOptionList filter={filter} options={options} />
	}),
	createInOrNotInFilter({
		name: 'Tags',
		icon: CircleDashed,
		extract: (arg) => {
			if ('object' in arg && 'tags' in arg.object) return arg.object.tags;
		},
		create: (tags) => ({ object: { tags } }),
		argsToOptions(values, options) {
			return values
				.map((value) => {
					const option = options.get(this.name)?.find((o) => o.value === value);

					if (!option) return;

					return {
						...option,
						type: this.name
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
		Render: ({ filter, options }) => <FilterOptionList filter={filter} options={options} />
	}),
	createInOrNotInFilter({
		name: 'Kind',
		icon: Cube,
		extract: (arg) => {
			if ('object' in arg && 'kind' in arg.object) return arg.object.kind;
		},
		create: (kind) => ({ object: { kind } }),
		argsToOptions(values, options) {
			return values
				.map((value) => {
					const option = options.get(this.name)?.find((o) => o.value === value);

					if (!option) return;

					return {
						...option,
						type: this.name
					};
				})
				.filter(Boolean) as any;
		},
		useOptions: () =>
			Object.keys(ObjectKind)
				.filter((key) => !isNaN(Number(key)) && ObjectKind[Number(key)] !== undefined)
				.map((key) => {
					const kind = ObjectKind[Number(key)];
					return {
						name: kind as string,
						value: Number(key),
						icon: kind + '20'
					};
				}),
		Render: ({ filter, options }) => <FilterOptionList filter={filter} options={options} />
	}),
	createTextMatchFilter({
		name: 'Name',
		icon: Textbox,
		extract: (arg) => {
			if ('filePath' in arg && 'name' in arg.filePath) return arg.filePath.name;
		},
		create: (name) => ({ filePath: { name } }),
		useOptions: ({ search }) => [{ name: search, value: search, icon: Textbox }],
		Render: ({ filter }) => <FilterOptionText filter={filter} />
	}),
	createInOrNotInFilter({
		name: 'Extension',
		icon: Textbox,
		extract: (arg) => {
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
		useOptions: ({ search }) => [{ name: search, value: search, icon: Textbox }],
		Render: ({ filter }) => <FilterOptionText filter={filter} />
	}),
	createBooleanFilter({
		name: 'Hidden',
		icon: SelectionSlash,
		extract: (arg) => {
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
		Render: ({ filter }) => <FilterOptionBoolean filter={filter} />
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
