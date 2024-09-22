import { TextMatch } from '@sd/client';

import { createFilter, CreateFilterFunction, filterTypeCondition, FilterTypeCondition } from '..';

/**
 * Creates a text match filter to handle search conditions such as `contains`, `startsWith`, `endsWith`, and `equals`.
 * This function leverages the generic factory structure to keep the logic reusable and consistent.
 *
 * @param filter - The initial filter configuration, including the create method, argsToFilterOptions, and other specific behaviors.
 * @returns A filter object that supports CRUD operations for text matching conditions.
 */
export function createTextMatchFilter(
	filter: CreateFilterFunction<FilterTypeCondition['textMatch'], TextMatch>
): ReturnType<typeof createFilter<FilterTypeCondition['textMatch'], TextMatch>> {
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

			return { [condition]: value };
		},

		argsToFilterOptions: (data) => {
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

		applyAdd: (data, { value }) => ({ contains: value }),

		applyRemove: () => undefined,

		merge: (left) => left
	};
}
