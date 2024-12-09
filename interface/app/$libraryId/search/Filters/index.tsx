/**
 * This module defines the logic for creating and managing search filters.
 * Please keep this index file clean and avoid adding any logic here.
 *
 * Instead of duplicating logic for every type of filter, we use generic factory patterns to create filters dynamically.
 * The core idea is to define reusable "conditions" for each filter type (e.g., `TextMatch`, `DateRange`, `InOrNotIn`) and
 * allow filters to be created via factory functions. The interface for CRUD operations remains the same across all filters,
 * but the condition logic varies depending on the type of filter.
 *
 * Key components:
 * - `SearchFilter`: Base interface for all filters.
 * - `SearchFilterCRUD`: Extends `SearchFilter` to handle conditions, CRUD operations, and UI rendering for filter options.
 * - `RenderSearchFilter`: Extends `SearchFilterCRUD` with rendering logic specific to each filter type.
 * - `createFilter`: A factory function to instantiate filters dynamically.
 * - `CreateFilterFunction`: A utility type for defining the structure of filter factories.
 *
 * This system allows the easy addition of new filters without repeating logic.
 */
import { Icon } from '@phosphor-icons/react';
import { SearchFilterArgs } from '@sd/client';
import i18n from '~/app/I18n';

import { UseSearch } from '../useSearch';
import { AllKeys, type FilterOption } from './store';
import { OmitCommonFilterProperties } from './typeGuards';

export { filterRegistry, type FilterType } from './FilterRegistry';

export type { FilterOption };

export { useToggleOptionSelected } from './hooks/useToggleOptionSelected';

// Base interface for any search filter
export interface SearchFilter<
	TConditions extends FilterTypeCondition[keyof FilterTypeCondition] = any
> {
	name: string;
	icon: Icon;
	conditions: TConditions;
	translationKey?: string;
}

// Extended interface for filters supporting CRUD operations
export interface SearchFilterCRUD<
	TConditions extends FilterTypeCondition[keyof FilterTypeCondition] = any, // Available conditions for the filter
	T = any // The data type being filtered
> extends SearchFilter<TConditions> {
	getCondition: (args: T) => AllKeys<TConditions>; // Gets the current filter condition
	setCondition: (args: T, condition: keyof TConditions) => void; // Sets a specific condition
	applyAdd: (args: T, option: FilterOption) => void; // Adds a filter option
	applyRemove: (args: T, option: FilterOption) => T | undefined; // Removes a filter option
	argsToFilterOptions: (args: T, options: Map<string, FilterOption[]>) => FilterOption[]; // Converts args to options for UI
	extract: (arg: SearchFilterArgs) => T | undefined; // Extracts relevant filter data
	create: (data: any) => SearchFilterArgs; // Creates a new filter argument
	merge: (left: T, right: T) => T; // Merges two sets of filter args
}

// Renderable search filter interface
export interface RenderSearchFilter<
	TConditions extends FilterTypeCondition[keyof FilterTypeCondition] = any,
	T = any
> extends SearchFilterCRUD<TConditions, T> {
	Render: (props: {
		filter: SearchFilterCRUD<TConditions>;
		options: (FilterOption & { type: string })[];
		search: UseSearch<any>;
	}) => JSX.Element;
	useOptions: (props: { search: string }) => FilterOption[];
}

// Factory function to create filters dynamically
export function createFilter<TConditions extends FilterTypeCondition[keyof FilterTypeCondition], T>(
	filter: RenderSearchFilter<TConditions, T>
) {
	return filter;
}

// Interface for filters that handle the `create` method
export interface FilterWithCreate<T, Value> {
	create: (value: Value) => SearchFilterArgs;
	argsToFilterOptions?: (values: T[], options: Map<string, FilterOption[]>) => FilterOption[];
}

// General factory type for creating filters
export type CreateFilterFunction<
	Conditions extends FilterTypeCondition[keyof FilterTypeCondition],
	Value
> = OmitCommonFilterProperties<ReturnType<typeof createFilter<Conditions, Value>>> &
	FilterWithCreate<any, Value>;

export const filterTypeCondition = {
	inOrNotIn: {
		in: i18n.t('is'),
		notIn: i18n.t('is_not')
	},
	textMatch: {
		contains: i18n.t('contains'),
		startsWith: i18n.t('starts_with'),
		endsWith: i18n.t('ends_with'),
		equals: i18n.t('equals')
	},
	trueOrFalse: {
		true: i18n.t('is'),
		false: i18n.t('is_not')
	},
	dateRange: {
		from: i18n.t('from'),
		to: i18n.t('to')
	}
} as const;

export type FilterTypeCondition = typeof filterTypeCondition;
