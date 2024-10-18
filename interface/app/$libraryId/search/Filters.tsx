import {
	CircleDashed,
	Cube,
	Folder,
	Heart,
	Icon,
	SelectionSlash,
	Textbox
} from '@phosphor-icons/react';
import { keepPreviousData } from '@tanstack/react-query';
import { useState } from 'react';
import { InOrNotIn, ObjectKind, SearchFilterArgs, TextMatch, useLibraryQuery } from '@sd/client';
import { Button, Input } from '@sd/ui';
import i18n from '~/app/I18n';
import { Icon as SDIcon } from '~/components';
import { useLocale } from '~/hooks';

import { SearchOptionItem, SearchOptionSubMenu } from '.';
import { translateKindName } from '../Explorer/util';
import { AllKeys, FilterOption, getKey } from './store';
import { UseSearch } from './useSearch';
import { FilterTypeCondition, filterTypeCondition } from './util';

export interface SearchFilter<
	TConditions extends FilterTypeCondition[keyof FilterTypeCondition] = any
> {
	name: string;
	icon: Icon;
	conditions: TConditions;
	translationKey?: string;
}

export interface SearchFilterCRUD<
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
		search: UseSearch<any>;
	}) => JSX.Element;
	// Apply is responsible for applying the filter to the search args
	useOptions: (props: { search: string }) => FilterOption[];
}

export function useToggleOptionSelected({ search }: { search: UseSearch<any> }) {
	return ({
		filter,
		option,
		select
	}: {
		filter: SearchFilterCRUD;
		option: FilterOption;
		select: boolean;
	}) => {
		search.setFilters?.((filters = []) => {
			const rawArg = filters.find((arg) => filter.extract(arg));

			if (!rawArg) {
				const arg = filter.create(option.value);
				filters.push(arg);
			} else {
				const rawArgIndex = filters.findIndex((arg) => filter.extract(arg))!;

				const arg = filter.extract(rawArg)!;

				if (select) {
					if (rawArg) filter.applyAdd(arg, option);
				} else {
					if (!filter.applyRemove(arg, option)) filters.splice(rawArgIndex, 1);
				}
			}

			return filters;
		});
	};
}

const FilterOptionList = ({
	filter,
	options,
	search,
	empty
}: {
	filter: SearchFilterCRUD;
	options: FilterOption[];
	search: UseSearch<any>;
	empty?: () => JSX.Element;
}) => {
	const { allFiltersKeys } = search;

	const toggleOptionSelected = useToggleOptionSelected({ search });

	return (
		<SearchOptionSubMenu name={filter.name} icon={filter.icon}>
			{empty?.() && options.length === 0
				? empty()
				: options?.map((option) => {
						const optionKey = getKey({
							...option,
							type: filter.name
						});

						return (
							<SearchOptionItem
								selected={allFiltersKeys.has(optionKey)}
								setSelected={(value) => {
									toggleOptionSelected({
										filter,
										option,
										select: value
									});
								}}
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

const FilterOptionText = ({
	filter,
	search
}: {
	filter: SearchFilterCRUD;
	search: UseSearch<any>;
}) => {
	const [value, setValue] = useState('');

	const { allFiltersKeys } = search;
	const key = getKey({
		type: filter.name,
		name: value,
		value
	});

	const { t } = useLocale();

	return (
		<SearchOptionSubMenu className="!p-1.5" name={filter.name} icon={filter.icon}>
			<form
				className="flex gap-1.5"
				onSubmit={(e) => {
					e.preventDefault();
					search.setFilters?.((filters) => {
						if (allFiltersKeys.has(key)) return filters;

						const arg = filter.create(value);
						filters?.push(arg);
						setValue('');

						return filters;
					});
				}}
			>
				<Input className="w-3/4" value={value} onChange={(e) => setValue(e.target.value)} />
				<Button
					disabled={value.length === 0 || allFiltersKeys.has(key)}
					variant="accent"
					className="w-full"
					type="submit"
				>
					{t('apply')}
				</Button>
			</form>
		</SearchOptionSubMenu>
	);
};

const FilterOptionBoolean = ({
	filter,
	search
}: {
	filter: SearchFilterCRUD;
	search: UseSearch<any>;
}) => {
	const { allFiltersKeys } = search;

	const key = getKey({
		type: filter.name,
		name: filter.name,
		value: true
	});

	return (
		<SearchOptionItem
			icon={filter.icon}
			selected={allFiltersKeys?.has(key)}
			setSelected={() => {
				search.setFilters?.((filters = []) => {
					const index = filters.findIndex((f) => filter.extract(f) !== undefined);

					if (index !== -1) {
						filters.splice(index, 1);
					} else {
						const arg = filter.create(true);
						filters.push(arg);
					}

					return filters;
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
			if ('in' in data) data.in = [...new Set([...data.in, option.value])];
			else data.notIn = [...new Set([...data.notIn, option.value])];

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
		name: i18n.t('location'),
		translationKey: 'location',
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
			const query = useLibraryQuery(['locations.list'], {
				placeholderData: keepPreviousData
			});
			const locations = query.data;

			return (locations ?? []).map((location) => ({
				name: location.name!,
				value: location.id,
				icon: 'Folder' // Spacedrive folder icon
			}));
		},
		Render: ({ filter, options, search }) => (
			<FilterOptionList filter={filter} options={options} search={search} />
		)
	}),
	createInOrNotInFilter({
		name: i18n.t('tags'),
		translationKey: 'tag',
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
			const query = useLibraryQuery(['tags.list']);
			const tags = query.data;
			return (tags ?? []).map((tag) => ({
				name: tag.name!,
				value: tag.id,
				icon: tag.color || 'CircleDashed'
			}));
		},
		Render: ({ filter, options, search }) => {
			return (
				<FilterOptionList
					empty={() => (
						<div className="flex flex-col items-center justify-center gap-2 p-2">
							<SDIcon name="Tags" size={32} />
							<p className="w-4/5 text-center text-xs text-ink-dull">
								{i18n.t('no_tags')}
							</p>
						</div>
					)}
					filter={filter}
					options={options}
					search={search}
				/>
			);
		}
	}),
	createInOrNotInFilter({
		name: i18n.t('kind'),
		translationKey: 'kind',
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
					const kind = ObjectKind[Number(key)] as string;
					return {
						name: translateKindName(kind),
						value: Number(key),
						icon: kind + '20'
					};
				}),
		Render: ({ filter, options, search }) => (
			<FilterOptionList filter={filter} options={options} search={search} />
		)
	}),
	createTextMatchFilter({
		name: i18n.t('name'),
		translationKey: 'name',
		icon: Textbox,
		extract: (arg) => {
			if ('filePath' in arg && 'name' in arg.filePath) return arg.filePath.name;
		},
		create: (name) => ({ filePath: { name } }),
		useOptions: ({ search }) => [{ name: search, value: search, icon: Textbox }],
		Render: ({ filter, search }) => <FilterOptionText filter={filter} search={search} />
	}),
	createInOrNotInFilter({
		name: i18n.t('extension'),
		translationKey: 'extension',
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
		Render: ({ filter, search }) => <FilterOptionText filter={filter} search={search} />
	}),
	createBooleanFilter({
		name: i18n.t('hidden'),
		translationKey: 'hidden',
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
		Render: ({ filter, search }) => <FilterOptionBoolean filter={filter} search={search} />
	}),
	createBooleanFilter({
		name: i18n.t('favorite'),
		translationKey: 'favorite',
		icon: Heart,
		extract: (arg) => {
			if ('object' in arg && 'favorite' in arg.object) return arg.object.favorite;
		},
		create: (favorite) => ({ object: { favorite } }),
		useOptions: () => {
			return [
				{
					name: 'Favorite',
					value: true,
					icon: 'Heart' // Spacedrive folder icon
				}
			];
		},
		Render: ({ filter, search }) => <FilterOptionBoolean filter={filter} search={search} />
	})
	// createInOrNotInFilter({
	// 	name: i18n.t('label'),
	// 	icon: Tag,
	// 	extract: (arg) => {
	// 		if ('object' in arg && 'labels' in arg.object) return arg.object.labels;
	// 	},
	// 	create: (labels) => ({ object: { labels } }),
	// 	argsToOptions(values, options) {
	// 		return values
	// 			.map((value) => {
	// 				const option = options.get(this.name)?.find((o) => o.value === value);

	// 				if (!option) return;

	// 				return {
	// 					...option,
	// 					type: this.name
	// 				};
	// 			})
	// 			.filter(Boolean) as any;
	// 	},
	// 	useOptions: () => {
	// 		const query = useLibraryQuery(['labels.list']);

	// 		return (query.data ?? []).map((label) => ({
	// 			name: label.name!,
	// 			value: label.id
	// 		}));
	// 	},
	// 	Render: ({ filter, options, search }) => (
	// 		<FilterOptionList filter={filter} options={options} search={search} />
	// 	)
	// })
	// idk how to handle this rn since include_descendants is part of 'path' now
	//
	// createFilter({
	// 	name: i18n.t('with_descendants'),
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
