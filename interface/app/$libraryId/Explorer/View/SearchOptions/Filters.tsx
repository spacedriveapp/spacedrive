import { CircleDashed, Cube, Folder, Icon, SelectionSlash, Textbox } from '@phosphor-icons/react';
import { produce } from 'immer';
import { useState } from 'react';
import { ObjectKind, SearchFilterArgs, TextMatch, useLibraryQuery } from '@sd/client';
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

export interface RenderSearchFilter<
	TConditions extends FilterTypeCondition[keyof FilterTypeCondition] = any
> extends SearchFilter<TConditions> {
	// Render is responsible for fetching the filter options and rendering them
	Render: (props: {
		filter: RenderSearchFilter<TConditions>;
		options: (FilterArgs & { type: string })[];
	}) => JSX.Element;
	// Apply is responsible for applying the filter to the search args
	apply: (filter: SetFilter, args: SearchFilterArgs) => void;
	useOptions: (props: { search: string }) => FilterArgs[];
	setCondition: (args: SearchFilterArgs, condition: keyof TConditions) => void;
	applyAdd: (args: SearchFilterArgs, option: FilterArgs) => void;
	applyRemove: (args: SearchFilterArgs, option: FilterArgs) => void;
	getCondition?: (args: SearchFilterArgs) => keyof TConditions | undefined;
	getOptionActive?: (args: SearchFilterArgs, option: FilterArgs) => boolean;
}

const FilterOptionList = ({
	filter,
	options
}: {
	filter: RenderSearchFilter;
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

function createFilter<TConditions extends FilterTypeCondition[keyof FilterTypeCondition]>(
	filter: RenderSearchFilter<TConditions>
) {
	return filter;
}

export const filterRegistry = [
	createFilter({
		name: 'Location',
		icon: Folder, // Phosphor folder icon
		conditions: filterTypeCondition.inOrNotIn,
		getCondition: (args) => {
			const locations = args.filePath?.locations;
			if (!locations) return;

			if ('in' in locations) return 'in';
			else return 'notIn';
		},
		getOptionActive: (args, option) => {
			const locations = args.filePath?.locations;
			if (!locations) return false;

			if ('in' in locations) return locations.in.includes(option.value);
			else return locations.notIn.includes(option.value);
		},
		setCondition: (args, condition) => {
			const filePath = (args.filePath ??= {});

			if (!filePath.locations)
				filePath.locations = condition === 'in' ? { in: [] } : { notIn: [] };
			else {
				let contents: number[];

				if ('in' in filePath.locations) contents = filePath.locations.in;
				else contents = filePath.locations.notIn;

				filePath.locations = condition === 'in' ? { in: contents } : { notIn: contents };
			}
		},
		applyAdd: (args, option) => {
			const locations = args.filePath?.locations;
			if (!locations) return;

			if ('in' in locations) locations.in.push(option.value);
			else locations.notIn.push(option.value);
		},
		applyRemove: (args, option) => {
			const filePath = args.filePath;
			if (!filePath?.locations) return;

			if ('in' in filePath.locations) {
				filePath.locations = {
					in: filePath.locations.in.filter((id) => id !== option.value)
				};

				if (filePath.locations.in.length === 0) delete filePath.locations;
			} else {
				filePath.locations = {
					notIn: filePath.locations.notIn.filter((id) => id !== option.value)
				};

				if (filePath.locations.notIn.length === 0) delete filePath.locations;
			}
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
		},
		apply: (filter, args) =>
			((args.filePath ??= {}).locations = inOrNotIn(
				args.filePath?.locations,
				filter.value,
				filter.condition
			))
	}),
	createFilter({
		name: 'Tags',
		icon: CircleDashed,
		conditions: filterTypeCondition.inOrNotIn,
		getCondition: (args) => {
			const tags = args.object?.tags;
			if (!tags) return;

			if ('in' in tags) return 'in';
			else return 'notIn';
		},
		getOptionActive: (args, option) => {
			const tags = args.object?.tags;
			if (!tags) return false;

			if ('in' in tags) return tags.in.includes(option.value);
			else return tags.notIn.includes(option.value);
		},
		setCondition: (args, condition: 'in' | 'notIn') => {
			const object = (args.object ??= {});

			if (!object.tags) object.tags = condition === 'in' ? { in: [] } : { notIn: [] };
			else {
				let contents: number[];

				if ('in' in object.tags) contents = object.tags.in;
				else contents = object.tags.notIn;

				object.tags = condition === 'in' ? { in: contents } : { notIn: contents };
			}
		},
		applyAdd: (args, option) => {
			const tags = args.object?.tags;
			if (!tags) return;

			if ('in' in tags) tags.in.push(option.value);
			else tags.notIn.push(option.value);
		},
		applyRemove: (args, option) => {
			const object = args.object;
			if (!object?.tags) return;

			if ('in' in object.tags) {
				object.tags = { in: object.tags.in.filter((id) => id !== option.value) };

				if (object.tags.in.length === 0) delete object.tags;
			} else {
				object.tags = { notIn: object.tags.notIn.filter((id) => id !== option.value) };

				if (object.tags.notIn.length === 0) delete object.tags;
			}
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
		},
		apply: (filter, args) => {
			(args.object ??= {}).tags = inOrNotIn(
				args.object?.tags,
				filter.value,
				filter.condition
			);
		}
	}),
	createFilter({
		name: 'Kind',
		icon: Cube,
		conditions: filterTypeCondition.inOrNotIn,
		setCondition: (args, condition: 'in' | 'notIn') => {
			const object = (args.object ??= {});

			if (!object.kind) object.kind = condition === 'in' ? { in: [] } : { notIn: [] };
			else {
				let contents: number[];

				if ('in' in object.kind) contents = object.kind.in;
				else contents = object.kind.notIn;

				object.kind = condition === 'in' ? { in: contents } : { notIn: contents };
			}
		},
		applyAdd: (args, option) => {
			const kind = args.object?.kind;
			if (!kind) return;

			if ('in' in kind) kind.in.push(option.value);
			else kind.notIn.push(option.value);
		},
		applyRemove: (args, option: FilterArgs) => {
			const object = args.object;
			if (!object?.kind) return;

			if ('in' in object.kind) {
				object.kind = { in: object.kind.in.filter((id) => id !== option.value) };
			} else {
				object.kind = { notIn: object.kind.notIn.filter((id) => id !== option.value) };
			}
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
		},
		apply: (filter, args) => {
			(args.object ??= {}).kind = inOrNotIn(
				args.object?.kind,
				filter.value,
				filter.condition
			);
		}
	}),
	createFilter({
		name: 'Name',
		icon: Textbox,
		conditions: filterTypeCondition.textMatch,
		setCondition: (args, condition: AllKeys<TextMatch>) => {
			const filePath = (args.filePath ??= {});

			let value = '';

			const name = filePath.name;

			if (name) {
				if ('contains' in name) value = name.contains;
				else if ('startsWith' in name) value = name.startsWith;
				else if ('endsWith' in name) value = name.endsWith;
				else if ('equals' in name) value = name.equals;
			}

			filePath.name = {
				[condition]: value
			} as TextMatch;
		},
		applyAdd: (args, option: FilterArgs) => {
			const name = args.filePath?.name;
			if (!name) return;

			if ('contains' in name) name.contains = option.value;
			else if ('startsWith' in name) name.startsWith = option.value;
			else if ('endsWith' in name) name.endsWith = option.value;
			else if ('equals' in name) name.equals = option.value;
		},
		applyRemove: (args) => {
			delete args.filePath?.name;
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
		},
		apply: (filter, args) => {
			(args.filePath ??= {}).name = textMatch('contains')(filter.value);
		}
	}),
	createFilter({
		name: 'Extension',
		icon: Textbox,
		conditions: filterTypeCondition.inOrNotIn,
		setCondition: (args, condition: 'in' | 'notIn') => {
			const filePath = (args.filePath ??= {});

			if (!filePath.extension)
				filePath.extension = condition === 'in' ? { in: [] } : { notIn: [] };
			else {
				let contents: string[];

				if ('in' in filePath.extension) contents = filePath.extension.in;
				else contents = filePath.extension.notIn;

				filePath.extension = condition === 'in' ? { in: contents } : { notIn: contents };
			}
		},
		applyAdd: (args, option: FilterArgs) => {
			const extension = args.filePath?.extension;
			if (!extension) return;

			if ('in' in extension) extension.in.push(option.value);
			else extension.notIn.push(option.value);
		},
		applyRemove: (args, option: FilterArgs) => {
			const filePath = args.filePath;
			if (!filePath?.extension) return;

			if ('in' in filePath.extension) {
				filePath.extension = {
					in: filePath.extension.in.filter((id) => id !== option.value)
				};
			} else {
				filePath.extension = {
					notIn: filePath.extension.notIn.filter((id) => id !== option.value)
				};
			}
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
	}),
	createFilter({
		name: 'Hidden',
		icon: SelectionSlash,
		conditions: filterTypeCondition.trueOrFalse,
		setCondition: (args, condition: 'true' | 'false') => {
			const filePath = (args.filePath ??= {});

			filePath.hidden = condition === 'true';
		},
		applyAdd: () => {},
		applyRemove: (args) => {
			delete args.filePath?.hidden;
		},
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
		},
		apply(filter, args) {
			(args.filePath ??= {}).hidden = filter.condition;
		}
	}),
	createFilter({
		name: 'WithDescendants',
		icon: SelectionSlash,
		conditions: filterTypeCondition.trueOrFalse,
		setCondition: (args, condition: 'true' | 'false') => {
			const filePath = (args.filePath ??= {});

			filePath.withDescendants = condition === 'true';
		},
		applyAdd: () => {},
		applyRemove: (args) => {
			delete args.filePath?.withDescendants;
		},
		useOptions: () => {
			return [
				{
					name: 'With Descendants',
					value: true,
					icon: 'SelectionSlash' // Spacedrive folder icon
				}
			];
		},
		Render: ({ filter }) => {
			return <FilterOptionBoolean filter={filter} />;
		},
		apply(filter, args) {
			(args.filePath ??= {}).withDescendants = filter.condition;
		}
	})
] as const satisfies ReadonlyArray<RenderSearchFilter<any>>;

export type FilterType = (typeof filterRegistry)[number]['name'];
