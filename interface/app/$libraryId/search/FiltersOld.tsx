// import {
// 	Calendar,
// 	CircleDashed,
// 	Cube,
// 	Folder,
// 	Heart,
// 	Icon,
// 	SelectionSlash,
// 	Textbox
// } from '@phosphor-icons/react';
// import { useState } from 'react';
// import {
// 	InOrNotIn,
// 	ObjectKind,
// 	Range,
// 	SearchFilterArgs,
// 	TextMatch,
// 	useLibraryQuery
// } from '@sd/client';
// import { Button, Input } from '@sd/ui';
// import i18n from '~/app/I18n';
// import { Icon as SDIcon } from '~/components';
// import { useLocale } from '~/hooks';

// import { SearchOptionItem, SearchOptionSubMenu } from '.';
// import { translateKindName } from '../Explorer/util';
// import { FilterTypeCondition, filterTypeCondition } from './FiltersOld';
// import { AllKeys, FilterOption, getKey } from './store';
// import { UseSearch } from './useSearch';

// interface SearchFilter<
// 	TConditions extends FilterTypeCondition[keyof FilterTypeCondition] = any
// > {
// 	name: string;
// 	icon: Icon;
// 	conditions: TConditions;
// 	translationKey?: string;
// }

// interface SearchFilterCRUD<
// 	TConditions extends FilterTypeCondition[keyof FilterTypeCondition] = any, // TConditions represents the available conditions for a specific filter, it defaults to any condition from the FilterTypeCondition
// 	T = any // T is the type of the data that is being filtered. This can be any type.
// > extends SearchFilter<TConditions> {
// 	// Extends the base SearchFilter interface, adding CRUD operations specific to handling filters

// 	// Returns the current filter condition for a given set of arguments (args).
// 	// This is used to determine which condition the filter is currently using (e.g., in, out, equals).
// 	getCondition: (args: T) => AllKeys<TConditions>;

// 	// Sets a specific filter condition (e.g., in, out, equals) for the given arguments (args).
// 	// The condition will be one of the predefined conditions in TConditions.
// 	setCondition: (args: T, condition: keyof TConditions) => void;

// 	// Adds a filter option to the current filter.
// 	// For example, if you are adding a tag, this method adds that tag to the filter’s arguments (args).
// 	applyAdd: (args: T, option: FilterOption) => void;

// 	// Removes a filter option from the current filter.
// 	// For example, if you are removing a tag, this method removes that tag from the filter's arguments (args).
// 	// Returns undefined if there are no more valid filters after removal.
// 	applyRemove: (args: T, option: FilterOption) => T | undefined;

// 	// Converts the filter arguments (args) into filter options that can be rendered in the UI.
// 	// It maps the provided arguments to an array of FilterOption objects, which are typically used in the dropdown or selectable options UI.
// 	argsToOptions: (args: T, options: Map<string, FilterOption[]>) => FilterOption[];

// 	// Extracts the relevant filter data from the larger SearchFilterArgs structure.
// 	// This is used to isolate the specific part of the filter (e.g., tag filter, date filter) that this filter instance is responsible for.
// 	extract: (arg: SearchFilterArgs) => T | undefined;

// 	// Creates a new SearchFilterArgs object based on the provided data.
// 	// This method builds the arguments used to represent the filter in the search request.
// 	create: (data: any) => SearchFilterArgs;

// 	// Merges two sets of filter arguments (left and right) into one.
// 	// This is useful when combining two different filter conditions for the same filter (e.g., merging two date ranges or tag selections).
// 	merge: (left: T, right: T) => T;
// }

// interface RenderSearchFilter<
// 	TConditions extends FilterTypeCondition[keyof FilterTypeCondition] = any,
// 	T = any
// > extends SearchFilterCRUD<TConditions, T> {
// 	// Render is responsible for fetching the filter options and rendering them
// 	Render: (props: {
// 		filter: SearchFilterCRUD<TConditions>;
// 		options: (FilterOption & { type: string })[];
// 		search: UseSearch<any>;
// 	}) => JSX.Element;
// 	// Apply is responsible for applying the filter to the search args
// 	useOptions: (props: { search: string }) => FilterOption[];
// }

// function useToggleOptionSelected({ search }: { search: UseSearch<any> }) {
// 	return ({
// 		filter,
// 		option,
// 		select
// 	}: {
// 		filter: SearchFilterCRUD;
// 		option: FilterOption;
// 		select: boolean;
// 	}) => {
// 		search.setFilters?.((filters = []) => {
// 			const rawArg = filters.find((arg) => filter.extract(arg));

// 			if (!rawArg) {
// 				const arg = filter.create(option.value);
// 				filters.push(arg);
// 			} else {
// 				const rawArgIndex = filters.findIndex((arg) => filter.extract(arg))!;

// 				const arg = filter.extract(rawArg)!;

// 				if (select) {
// 					if (rawArg) filter.applyAdd(arg, option);
// 				} else {
// 					if (!filter.applyRemove(arg, option)) filters.splice(rawArgIndex, 1);
// 				}
// 			}

// 			return filters;
// 		});
// 	};
// }

// const FilterOptionList = ({
// 	filter,
// 	options,
// 	search,
// 	empty
// }: {
// 	filter: SearchFilterCRUD;
// 	options: FilterOption[];
// 	search: UseSearch<any>;
// 	empty?: () => JSX.Element;
// }) => {
// 	const { allFiltersKeys } = search;

// 	const toggleOptionSelected = useToggleOptionSelected({ search });

// 	return (
// 		<SearchOptionSubMenu name={filter.name} icon={filter.icon}>
// 			{empty?.() && options.length === 0
// 				? empty()
// 				: options?.map((option) => {
// 						const optionKey = getKey({
// 							...option,
// 							type: filter.name
// 						});

// 						return (
// 							<SearchOptionItem
// 								selected={allFiltersKeys.has(optionKey)}
// 								setSelected={(value) => {
// 									toggleOptionSelected({
// 										filter,
// 										option,
// 										select: value
// 									});
// 								}}
// 								key={option.value}
// 								icon={option.icon}
// 							>
// 								{option.name}
// 							</SearchOptionItem>
// 						);
// 					})}
// 		</SearchOptionSubMenu>
// 	);
// };

// const FilterOptionText = ({
// 	filter,
// 	search
// }: {
// 	filter: SearchFilterCRUD;
// 	search: UseSearch<any>;
// }) => {
// 	const [value, setValue] = useState('');

// 	const { allFiltersKeys } = search;
// 	const key = getKey({
// 		type: filter.name,
// 		name: value,
// 		value
// 	});

// 	const { t } = useLocale();

// 	return (
// 		<SearchOptionSubMenu className="!p-1.5" name={filter.name} icon={filter.icon}>
// 			<form
// 				className="flex gap-1.5"
// 				onSubmit={(e) => {
// 					e.preventDefault();
// 					search.setFilters?.((filters) => {
// 						if (allFiltersKeys.has(key)) return filters;

// 						const arg = filter.create(value);
// 						filters?.push(arg);
// 						setValue('');

// 						return filters;
// 					});
// 				}}
// 			>
// 				<Input className="w-3/4" value={value} onChange={(e) => setValue(e.target.value)} />
// 				<Button
// 					disabled={value.length === 0 || allFiltersKeys.has(key)}
// 					variant="accent"
// 					className="w-full"
// 					type="submit"
// 				>
// 					{t('apply')}
// 				</Button>
// 			</form>
// 		</SearchOptionSubMenu>
// 	);
// };

// const FilterOptionBoolean = ({
// 	filter,
// 	search
// }: {
// 	filter: SearchFilterCRUD;
// 	search: UseSearch<any>;
// }) => {
// 	const { allFiltersKeys } = search;

// 	const key = getKey({
// 		type: filter.name,
// 		name: filter.name,
// 		value: true
// 	});

// 	return (
// 		<SearchOptionItem
// 			icon={filter.icon}
// 			selected={allFiltersKeys?.has(key)}
// 			setSelected={() => {
// 				search.setFilters?.((filters = []) => {
// 					const index = filters.findIndex((f) => filter.extract(f) !== undefined);

// 					if (index !== -1) {
// 						filters.splice(index, 1);
// 					} else {
// 						const arg = filter.create(true);
// 						filters.push(arg);
// 					}

// 					return filters;
// 				});
// 			}}
// 		>
// 			{filter.name}
// 		</SearchOptionItem>
// 	);
// };

// function createFilter<TConditions extends FilterTypeCondition[keyof FilterTypeCondition], T>(
// 	filter: RenderSearchFilter<TConditions, T>
// ) {
// 	return filter;
// }

// function createInOrNotInFilter<T extends string | number>(
// 	filter: Omit<
// 		ReturnType<typeof createFilter<any, InOrNotIn<T>>>,
// 		| 'conditions'
// 		| 'getCondition'
// 		| 'argsToOptions'
// 		| 'setCondition'
// 		| 'applyAdd'
// 		| 'applyRemove'
// 		| 'create'
// 		| 'merge'
// 	> & {
// 		create(value: InOrNotIn<T>): SearchFilterArgs;
// 		argsToOptions(values: T[], options: Map<string, FilterOption[]>): FilterOption[];
// 	}
// ): ReturnType<typeof createFilter<(typeof filterTypeCondition)['inOrNotIn'], InOrNotIn<T>>> {
// 	return {
// 		...filter,
// 		create: (data) => {
// 			if (typeof data === 'number' || typeof data === 'string')
// 				return filter.create({
// 					in: [data as any]
// 				});
// 			else if (data) return filter.create(data);
// 			else return filter.create({ in: [] });
// 		},
// 		conditions: filterTypeCondition.inOrNotIn,
// 		getCondition: (data) => {
// 			if ('in' in data) return 'in';
// 			else return 'notIn';
// 		},
// 		setCondition: (data, condition) => {
// 			const contents = 'in' in data ? data.in : data.notIn;

// 			return condition === 'in' ? { in: contents } : { notIn: contents };
// 		},
// 		argsToOptions: (data, options) => {
// 			let values: T[];

// 			if ('in' in data) values = data.in;
// 			else values = data.notIn;

// 			return filter.argsToOptions(values, options);
// 		},
// 		applyAdd: (data, option) => {
// 			if ('in' in data) data.in = [...new Set([...data.in, option.value])];
// 			else data.notIn = [...new Set([...data.notIn, option.value])];

// 			return data;
// 		},
// 		applyRemove: (data, option) => {
// 			if ('in' in data) {
// 				data.in = data.in.filter((id) => id !== option.value);

// 				if (data.in.length === 0) return;
// 			} else {
// 				data.notIn = data.notIn.filter((id) => id !== option.value);

// 				if (data.notIn.length === 0) return;
// 			}

// 			return data;
// 		},
// 		merge: (left, right) => {
// 			if ('in' in left && 'in' in right) {
// 				return {
// 					in: [...new Set([...left.in, ...right.in])]
// 				};
// 			} else if ('notIn' in left && 'notIn' in right) {
// 				return {
// 					notIn: [...new Set([...left.notIn, ...right.notIn])]
// 				};
// 			}

// 			throw new Error('Cannot merge InOrNotIns with different conditions');
// 		}
// 	};
// }

// function createTextMatchFilter(
// 	filter: Omit<
// 		ReturnType<typeof createFilter<any, TextMatch>>,
// 		| 'conditions'
// 		| 'getCondition'
// 		| 'argsToOptions'
// 		| 'setCondition'
// 		| 'applyAdd'
// 		| 'applyRemove'
// 		| 'create'
// 		| 'merge'
// 	> & {
// 		create(value: TextMatch): SearchFilterArgs;
// 	}
// ): ReturnType<typeof createFilter<(typeof filterTypeCondition)['textMatch'], TextMatch>> {
// 	return {
// 		...filter,
// 		conditions: filterTypeCondition.textMatch,
// 		create: (contains) => filter.create({ contains }),
// 		getCondition: (data) => {
// 			if ('contains' in data) return 'contains';
// 			else if ('startsWith' in data) return 'startsWith';
// 			else if ('endsWith' in data) return 'endsWith';
// 			else return 'equals';
// 		},
// 		setCondition: (data, condition) => {
// 			let value: string;

// 			if ('contains' in data) value = data.contains;
// 			else if ('startsWith' in data) value = data.startsWith;
// 			else if ('endsWith' in data) value = data.endsWith;
// 			else value = data.equals;

// 			return {
// 				[condition]: value
// 			};
// 		},
// 		argsToOptions: (data) => {
// 			let value: string;

// 			if ('contains' in data) value = data.contains;
// 			else if ('startsWith' in data) value = data.startsWith;
// 			else if ('endsWith' in data) value = data.endsWith;
// 			else value = data.equals;

// 			return [
// 				{
// 					type: filter.name,
// 					name: value,
// 					value
// 				}
// 			];
// 		},
// 		applyAdd: (data, { value }) => {
// 			if ('contains' in data) return { contains: value };
// 			else if ('startsWith' in data) return { startsWith: value };
// 			else if ('endsWith' in data) return { endsWith: value };
// 			else if ('equals' in data) return { equals: value };
// 		},
// 		applyRemove: () => undefined,
// 		merge: (left) => left
// 	};
// }

// function createBooleanFilter(
// 	filter: Omit<
// 		ReturnType<typeof createFilter<any, boolean>>,
// 		| 'conditions'
// 		| 'getCondition'
// 		| 'argsToOptions'
// 		| 'setCondition'
// 		| 'applyAdd'
// 		| 'applyRemove'
// 		| 'create'
// 		| 'merge'
// 	> & {
// 		create(value: boolean): SearchFilterArgs;
// 	}
// ): ReturnType<typeof createFilter<(typeof filterTypeCondition)['trueOrFalse'], boolean>> {
// 	return {
// 		...filter,
// 		conditions: filterTypeCondition.trueOrFalse,
// 		create: () => filter.create(true),
// 		getCondition: (data) => (data ? 'true' : 'false'),
// 		setCondition: (_, condition) => condition === 'true',
// 		argsToOptions: (value) => {
// 			if (!value) return [];

// 			return [
// 				{
// 					type: filter.name,
// 					name: filter.name,
// 					value
// 				}
// 			];
// 		},
// 		applyAdd: (_, { value }) => value,
// 		applyRemove: () => undefined,
// 		merge: (left) => left
// 	};
// }

// function createRangeFilter<T>(
// 	filter: Omit<
// 		ReturnType<typeof createFilter<any, Range<T>>>,
// 		| 'conditions'
// 		| 'getCondition'
// 		| 'argsToOptions'
// 		| 'setCondition'
// 		| 'applyAdd'
// 		| 'applyRemove'
// 		| 'create'
// 		| 'merge'
// 	> & {
// 		create(value: Range<T>): SearchFilterArgs;
// 		argsToOptions(values: T[], options: Map<string, FilterOption[]>): FilterOption[];
// 	}
// ): ReturnType<typeof createFilter<(typeof filterTypeCondition)['range'], Range<T>>> {
// 	return {
// 		...filter,
// 		conditions: filterTypeCondition.range,
// 		create: (data) => {
// 			if ('from' in data) {
// 				return filter.create({ from: data.from });
// 			} else if ('to' in data) {
// 				return filter.create({ to: data.to });
// 			} else {
// 				throw new Error('Invalid Range data');
// 			}
// 		},
// 		getCondition: (data) => {
// 			if ('from' in data) return 'from';
// 			else if ('to' in data) return 'to';
// 			else throw new Error('Invalid Range data');
// 		},
// 		setCondition: (data, condition) => {
// 			return condition === 'from' && 'from' in data
// 				? { from: data.from }
// 				: condition === 'to' && 'to' in data
// 					? { to: data.to }
// 					: (() => {
// 							throw new Error('Invalid condition or missing data');
// 						})();
// 		},
// 		argsToOptions: (data, options) => {
// 			const values: T[] = [];
// 			if ('from' in data) values.push(data.from);
// 			if ('to' in data) values.push(data.to);

// 			return values.map((value) => ({
// 				type: filter.name,
// 				name: String(value),
// 				value
// 			}));
// 		},
// 		applyAdd: (data, option) => {
// 			if ('from' in data) {
// 				data.from = option.value;
// 			} else if ('to' in data) {
// 				data.to = option.value;
// 			} else {
// 				throw new Error('Invalid Range data');
// 			}
// 			return data;
// 		},
// 		applyRemove: (data, option): Range<T> | undefined => {
// 			if ('from' in data && data.from === option.value) {
// 				const { from, ...rest } = data; // Omit `from`
// 				return Object.keys(rest).length ? (rest as Range<T>) : undefined;
// 			} else if ('to' in data && data.to === option.value) {
// 				const { to, ...rest } = data; // Omit `to`
// 				return Object.keys(rest).length ? (rest as Range<T>) : undefined;
// 			}

// 			return data;
// 		},
// 		merge: (left, right): Range<T> => {
// 			const result = {
// 				...('from' in left
// 					? { from: left.from }
// 					: 'from' in right
// 						? { from: right.from }
// 						: {}),
// 				...('to' in left ? { to: left.to } : 'to' in right ? { to: right.to } : {})
// 			};

// 			return result as Range<T>;
// 		}
// 	};
// }

// function createGenericRangeFilter<T>(
// 	name: string,
// 	translationKey: string,
// 	icon: Icon,
// 	extractFn: (arg: SearchFilterArgs) => Range<T> | undefined,
// 	createFn: (range: Range<T>) => SearchFilterArgs
// ): ReturnType<typeof createFilter<(typeof filterTypeCondition)['range'], Range<T>>> {
// 	return createRangeFilter({
// 		name,
// 		translationKey,
// 		icon,
// 		extract: extractFn,
// 		create: createFn,
// 		Render: ({ filter, options, search }) => (
// 			<FilterOptionList filter={filter} options={options} search={search} />
// 		),
// 		useOptions: (): FilterOption[] => {
// 			// Predefined date range options, or you can make it dynamic based on type T
// 			return [
// 				{
// 					name: 'Last 7 Days',
// 					value: { from: new Date(Date.now() - 7 * 24 * 60 * 60 * 1000).toISOString() },
// 					icon: Calendar
// 				},
// 				{
// 					name: 'Last 30 Days',
// 					value: { from: new Date(Date.now() - 30 * 24 * 60 * 60 * 1000).toISOString() },
// 					icon: Calendar
// 				},
// 				{
// 					name: 'This Year',
// 					value: { from: new Date(new Date().getFullYear(), 0, 1).toISOString() },
// 					icon: Calendar
// 				}
// 			];
// 		},
// 		argsToOptions: (values: T[], options: Map<string, FilterOption[]>): FilterOption[] => {
// 			return values.map((value) => ({
// 				type: name,
// 				name: String(value),
// 				value,
// 				icon: Calendar
// 			}));
// 		}
// 	});
// }

// const filterRegistry = [
// 	createGenericRangeFilter(
// 		i18n.t('date_created_range'),
// 		'date_created_range',
// 		Calendar,
// 		// extract
// 		(arg) => {
// 			if ('filePath' in arg && 'date_created' in arg.filePath) {
// 				return {
// 					from: arg.filePath.date_created,
// 					to: arg.filePath.date_created
// 				} as Range<string>;
// 			}
// 		},
// 		// create
// 		(dateRange: Range<string>) => {
// 			return {
// 				filePath: {
// 					createdAt: {
// 						from: 'from' in dateRange ? dateRange.from : undefined,
// 						to: 'to' in dateRange ? dateRange.to : undefined
// 					}
// 				}
// 			} as SearchFilterArgs;
// 		}
// 	),
// 	createGenericRangeFilter(
// 		i18n.t('date_accessed_range'),
// 		'date_accessed_range',
// 		Calendar,
// 		// extract
// 		(arg) => {
// 			if ('object' in arg && 'date_accessed' in arg.object) {
// 				return {
// 					from: arg.object.date_accessed,
// 					to: arg.object.date_accessed
// 				} as Range<string>;
// 			}
// 		},
// 		// create
// 		(dateRange: Range<string>) => {
// 			return {
// 				object: {
// 					dateAccessed: {
// 						from: 'from' in dateRange ? dateRange.from : undefined,
// 						to: 'to' in dateRange ? dateRange.to : undefined
// 					}
// 				}
// 			} as SearchFilterArgs;
// 		}
// 	),
// 	createInOrNotInFilter({
// 		name: i18n.t('location'),
// 		translationKey: 'location',
// 		icon: Folder, // Phosphor folder icon
// 		extract: (arg) => {
// 			if ('filePath' in arg && 'locations' in arg.filePath) return arg.filePath.locations;
// 		},
// 		create: (locations) => ({ filePath: { locations } }),
// 		argsToOptions(values, options) {
// 			return values
// 				.map((value) => {
// 					const option = options.get(this.name)?.find((o) => o.value === value);

// 					if (!option) return;

// 					return {
// 						...option,
// 						type: this.name
// 					};
// 				})
// 				.filter(Boolean) as any;
// 		},
// 		useOptions: () => {
// 			const query = useLibraryQuery(['locations.list'], { keepPreviousData: true });
// 			const locations = query.data;

// 			return (locations ?? []).map((location) => ({
// 				name: location.name!,
// 				value: location.id,
// 				icon: 'Folder' // Spacedrive folder icon
// 			}));
// 		},
// 		Render: ({ filter, options, search }) => (
// 			<FilterOptionList filter={filter} options={options} search={search} />
// 		)
// 	}),
// 	createInOrNotInFilter({
// 		name: i18n.t('tags'),
// 		translationKey: 'tag',
// 		icon: CircleDashed,
// 		extract: (arg) => {
// 			if ('object' in arg && 'tags' in arg.object) return arg.object.tags;
// 		},
// 		create: (tags) => ({ object: { tags } }),
// 		argsToOptions(values, options) {
// 			return values
// 				.map((value) => {
// 					const option = options.get(this.name)?.find((o) => o.value === value);

// 					if (!option) return;

// 					return {
// 						...option,
// 						type: this.name
// 					};
// 				})
// 				.filter(Boolean) as any;
// 		},
// 		useOptions: () => {
// 			const query = useLibraryQuery(['tags.list']);
// 			const tags = query.data;
// 			return (tags ?? []).map((tag) => ({
// 				name: tag.name!,
// 				value: tag.id,
// 				icon: tag.color || 'CircleDashed'
// 			}));
// 		},
// 		Render: ({ filter, options, search }) => {
// 			return (
// 				<FilterOptionList
// 					empty={() => (
// 						<div className="flex flex-col items-center justify-center gap-2 p-2">
// 							<SDIcon name="Tags" size={32} />
// 							<p className="w-4/5 text-center text-xs text-ink-dull">
// 								{i18n.t('no_tags')}
// 							</p>
// 						</div>
// 					)}
// 					filter={filter}
// 					options={options}
// 					search={search}
// 				/>
// 			);
// 		}
// 	}),
// 	createInOrNotInFilter({
// 		name: i18n.t('kind'),
// 		translationKey: 'kind',
// 		icon: Cube,
// 		extract: (arg) => {
// 			if ('object' in arg && 'kind' in arg.object) return arg.object.kind;
// 		},
// 		create: (kind) => ({ object: { kind } }),
// 		argsToOptions(values, options) {
// 			return values
// 				.map((value) => {
// 					const option = options.get(this.name)?.find((o) => o.value === value);

// 					if (!option) return;

// 					return {
// 						...option,
// 						type: this.name
// 					};
// 				})
// 				.filter(Boolean) as any;
// 		},
// 		useOptions: () =>
// 			Object.keys(ObjectKind)
// 				.filter((key) => !isNaN(Number(key)) && ObjectKind[Number(key)] !== undefined)
// 				.map((key) => {
// 					const kind = ObjectKind[Number(key)] as string;
// 					return {
// 						name: translateKindName(kind),
// 						value: Number(key),
// 						icon: kind + '20'
// 					};
// 				}),
// 		Render: ({ filter, options, search }) => (
// 			<FilterOptionList filter={filter} options={options} search={search} />
// 		)
// 	}),
// 	createTextMatchFilter({
// 		name: i18n.t('name'),
// 		translationKey: 'name',
// 		icon: Textbox,
// 		extract: (arg) => {
// 			if ('filePath' in arg && 'name' in arg.filePath) return arg.filePath.name;
// 		},
// 		create: (name) => ({ filePath: { name } }),
// 		useOptions: ({ search }) => [{ name: search, value: search, icon: Textbox }],
// 		Render: ({ filter, search }) => <FilterOptionText filter={filter} search={search} />
// 	}),
// 	createInOrNotInFilter({
// 		name: i18n.t('extension'),
// 		translationKey: 'extension',
// 		icon: Textbox,
// 		extract: (arg) => {
// 			if ('filePath' in arg && 'extension' in arg.filePath) return arg.filePath.extension;
// 		},
// 		create: (extension) => ({ filePath: { extension } }),
// 		argsToOptions(values) {
// 			return values.map((value) => ({
// 				type: this.name,
// 				name: value,
// 				value
// 			}));
// 		},
// 		useOptions: ({ search }) => [{ name: search, value: search, icon: Textbox }],
// 		Render: ({ filter, search }) => <FilterOptionText filter={filter} search={search} />
// 	}),
// 	createBooleanFilter({
// 		name: i18n.t('hidden'),
// 		translationKey: 'hidden',
// 		icon: SelectionSlash,
// 		extract: (arg) => {
// 			if ('filePath' in arg && 'hidden' in arg.filePath) return arg.filePath.hidden;
// 		},
// 		create: (hidden) => ({ filePath: { hidden } }),
// 		useOptions: () => {
// 			return [
// 				{
// 					name: 'Hidden',
// 					value: true,
// 					icon: 'SelectionSlash' // Spacedrive folder icon
// 				}
// 			];
// 		},
// 		Render: ({ filter, search }) => <FilterOptionBoolean filter={filter} search={search} />
// 	}),
// 	createBooleanFilter({
// 		name: i18n.t('favorite'),
// 		translationKey: 'favorite',
// 		icon: Heart,
// 		extract: (arg) => {
// 			if ('object' in arg && 'favorite' in arg.object) return arg.object.favorite;
// 		},
// 		create: (favorite) => ({ object: { favorite } }),
// 		useOptions: () => {
// 			return [
// 				{
// 					name: 'Favorite',
// 					value: true,
// 					icon: 'Heart' // Spacedrive folder icon
// 				}
// 			];
// 		},
// 		Render: ({ filter, search }) => <FilterOptionBoolean filter={filter} search={search} />
// 	})
// 	// createInOrNotInFilter({
// 	// 	name: i18n.t('label'),
// 	// 	icon: Tag,
// 	// 	extract: (arg) => {
// 	// 		if ('object' in arg && 'labels' in arg.object) return arg.object.labels;
// 	// 	},
// 	// 	create: (labels) => ({ object: { labels } }),
// 	// 	argsToOptions(values, options) {
// 	// 		return values
// 	// 			.map((value) => {
// 	// 				const option = options.get(this.name)?.find((o) => o.value === value);

// 	// 				if (!option) return;

// 	// 				return {
// 	// 					...option,
// 	// 					type: this.name
// 	// 				};
// 	// 			})
// 	// 			.filter(Boolean) as any;
// 	// 	},
// 	// 	useOptions: () => {
// 	// 		const query = useLibraryQuery(['labels.list']);

// 	// 		return (query.data ?? []).map((label) => ({
// 	// 			name: label.name!,
// 	// 			value: label.id
// 	// 		}));
// 	// 	},
// 	// 	Render: ({ filter, options, search }) => (
// 	// 		<FilterOptionList filter={filter} options={options} search={search} />
// 	// 	)
// 	// })
// 	// idk how to handle this rn since include_descendants is part of 'path' now
// 	//
// 	// createFilter({
// 	// 	name: i18n.t('with_descendants'),
// 	// 	icon: SelectionSlash,
// 	// 	conditions: filterTypeCondition.trueOrFalse,
// 	// 	setCondition: (args, condition: 'true' | 'false') => {
// 	// 		const filePath = (args.filePath ??= {});

// 	// 		filePath.withDescendants = condition === 'true';
// 	// 	},
// 	// 	applyAdd: () => {},
// 	// 	applyRemove: (args) => {
// 	// 		delete args.filePath?.withDescendants;
// 	// 	},
// 	// 	useOptions: () => {
// 	// 		return [
// 	// 			{
// 	// 				name: 'With Descendants',
// 	// 				value: true,
// 	// 				icon: 'SelectionSlash' // Spacedrive folder icon
// 	// 			}
// 	// 		];
// 	// 	},
// 	// 	Render: ({ filter }) => {
// 	// 		return <FilterOptionBoolean filter={filter} />;
// 	// 	},
// 	// 	apply(filter, args) {
// 	// 		(args.filePath ??= {}).withDescendants = filter.condition;
// 	// 	}
// 	// })
// ] as const satisfies ReadonlyArray<RenderSearchFilter<any>>;

// type FilterType = (typeof filterRegistry)[number]['name'];
