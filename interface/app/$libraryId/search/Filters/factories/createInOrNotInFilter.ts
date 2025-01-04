import { InOrNotIn } from '@sd/client';

import { createFilter, CreateFilterFunction, filterTypeCondition, FilterTypeCondition } from '..';

/**
 * Creates an "In or Not In" filter to handle conditions like `in` or `notIn`.
 * This function leverages the generic factory structure to keep the logic reusable and consistent.
 *
 * @param filter - The initial filter configuration, including the create method, argsToFilterOptions, and other specific behaviors.
 * @returns A filter object that supports CRUD operations for in/notIn conditions.
 */
export function createInOrNotInFilter<T extends string | number>(
	filter: CreateFilterFunction<FilterTypeCondition['inOrNotIn'], InOrNotIn<T>>
): ReturnType<typeof createFilter<FilterTypeCondition['inOrNotIn'], InOrNotIn<T>>> {
	return {
		...filter,
		conditions: filterTypeCondition.inOrNotIn,

		create: (data) => {
			if (typeof data === 'number' || typeof data === 'string') {
				return filter.create({ in: [data as any] });
			} else if (data) {
				return filter.create(data);
			} else {
				return filter.create({ in: [] });
			}
		},

		getCondition: (data) => {
			if ('in' in data) return 'in';
			else return 'notIn';
		},

		setCondition: (data, condition) => {
			const contents = 'in' in data ? data.in : data.notIn;
			return condition === 'in' ? { in: contents } : { notIn: contents };
		},

		argsToFilterOptions: (data, options) => {
			let values: T[];
			if ('in' in data) {
				values = data.in;
			} else {
				values = data.notIn;
			}
			if (filter.argsToFilterOptions) {
				return filter.argsToFilterOptions(values, options);
			}
			return [];
		},

		applyAdd: (data, option) => {
			if ('in' in data) {
				data.in = [...new Set([...data.in, option.value])];
			} else {
				data.notIn = [...new Set([...data.notIn, option.value])];
			}
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
				return { in: [...new Set([...left.in, ...right.in])] };
			} else if ('notIn' in left && 'notIn' in right) {
				return { notIn: [...new Set([...left.notIn, ...right.notIn])] };
			}
			throw new Error('Cannot merge InOrNotIns with different conditions');
		}
	};
}
