import { Range } from '@sd/client';

import { createFilter, CreateFilterFunction, filterTypeCondition, FilterTypeCondition } from '..';

/**
 * Creates a range filter to handle conditions such as `from` and `to`.
 * This function leverages the generic factory structure to keep the logic reusable and consistent.
 *
 * @param filter - The initial filter configuration, including the create method, argsToFilterOptions, and other specific behaviors.
 * @returns A filter object that supports CRUD operations for range conditions.
 */
export function createDateRangeFilter<T extends string | number>(
	filter: CreateFilterFunction<FilterTypeCondition['dateRange'], Range<T>>
): ReturnType<typeof createFilter<FilterTypeCondition['dateRange'], Range<T>>> {
	return {
		...filter,
		conditions: filterTypeCondition.dateRange,

		create: (data) => {
			if ('from' in data) {
				return filter.create({ from: data.from });
			} else if ('to' in data) {
				return filter.create({ to: data.to });
			} else {
				throw new Error('Invalid Range data');
			}
		},

		getCondition: (data) => {
			if ('from' in data) return 'from';
			else if ('to' in data) return 'to';
			else throw new Error('Invalid Range data');
		},

		setCondition: (data, condition) => {
			if (condition === 'from' && 'from' in data) {
				return { from: data.from };
			} else if (condition === 'to' && 'to' in data) {
				return { to: data.to };
			} else {
				throw new Error('Invalid condition or missing data');
			}
		},

		argsToFilterOptions: (data, options) => {
			const values: T[] = [];
			if ('from' in data) values.push(data.from);
			if ('to' in data) values.push(data.to);

			if (filter.argsToFilterOptions) {
				return filter.argsToFilterOptions(values, options);
			}

			return values.map((value) => ({
				type: filter.name,
				name: String(value),
				value
			}));
		},

		applyAdd: (data, option) => {
			if ('from' in data) {
				data.from = option.value;
			} else if ('to' in data) {
				data.to = option.value;
			} else {
				throw new Error('Invalid Range data');
			}
			return data;
		},

		applyRemove: (data, option): Range<T> | undefined => {
			if ('from' in data && data.from === option.value) {
				const { from, ...rest } = data; // Omit `from`
				return Object.keys(rest).length ? (rest as Range<T>) : undefined;
			} else if ('to' in data && data.to === option.value) {
				const { to, ...rest } = data; // Omit `to`
				return Object.keys(rest).length ? (rest as Range<T>) : undefined;
			}

			return data;
		},

		merge: (left, right): Range<T> => {
			return {
				...('from' in left ? { from: left.from } : {}),
				...('to' in left ? { to: left.to } : {}),
				...('from' in right ? { from: right.from } : {}),
				...('to' in right ? { to: right.to } : {})
			} as Range<T>;
		}
	};
}
