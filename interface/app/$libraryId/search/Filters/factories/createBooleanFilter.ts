import { createFilter, CreateFilterFunction, filterTypeCondition, FilterTypeCondition } from '..';

// TODO: Move these factories to @sd/client
/**
 * Creates a boolean filter to handle conditions like `true` or `false`.
 * This function leverages the generic factory structure to keep the logic reusable and consistent.
 *
 * @param filter - The initial filter configuration, including the create method, argsToFilterOptions, and other specific behaviors.
 * @returns A filter object that supports CRUD operations for boolean conditions.
 */
export function createBooleanFilter(
	filter: CreateFilterFunction<FilterTypeCondition['trueOrFalse'], boolean>
): ReturnType<typeof createFilter<FilterTypeCondition['trueOrFalse'], boolean>> {
	return {
		...filter,
		conditions: filterTypeCondition.trueOrFalse,

		create: (value: boolean) => filter.create(value),

		getCondition: (data) => (data ? 'true' : 'false'),

		setCondition: (_, condition) => condition === 'true',

		argsToFilterOptions: (data) => {
			if (filter.argsToFilterOptions) {
				return filter.argsToFilterOptions([data], new Map());
			}
			return [
				{
					type: filter.name,
					name: filter.name,
					value: data
				}
			];
		},

		applyAdd: (_, option) => option.value,

		applyRemove: () => undefined, // Boolean filters don't have multiple values, so nothing to remove

		merge: (left) => left // Boolean filters don't require merging; return the existing value
	};
}
