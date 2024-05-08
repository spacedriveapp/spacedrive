import { z } from 'zod';

import { SortOrder } from '../core';

export type Ordering = { field: string; value: SortOrder | Ordering };
// branded type for added type-safety
export type OrderingKey = string & {};

type OrderingValue<T extends Ordering, K extends string> = Extract<T, { field: K }>['value'];

export type OrderingKeys<T extends Ordering> = T extends Ordering
	? {
			[K in T['field']]: OrderingValue<T, K> extends SortOrder
				? K
				: OrderingValue<T, K> extends Ordering
					? `${K}.${OrderingKeys<OrderingValue<T, K>>}`
					: never;
		}[T['field']]
	: never;

export function getOrderingKey(ordering: Ordering): OrderingKey {
	let base = ordering.field;

	if (typeof ordering.value === 'object') {
		base += `.${getOrderingKey(ordering.value)}`;
	}

	return base;
}

export function createOrdering<TOrdering extends Ordering = Ordering>(
	key: OrderingKey,
	value: SortOrder
): TOrdering {
	return key
		.split('.')
		.reverse()
		.reduce((acc, field, i) => {
			if (i === 0)
				return {
					field,
					value
				};
			else return { field, value: acc };
		}, {} as any);
}

export function getOrderingDirection(ordering: Ordering): SortOrder {
	if (typeof ordering.value === 'object') return getOrderingDirection(ordering.value);
	else return ordering.value;
}

export const filePathOrderingKeysSchema = z.union([
	z.literal('name').describe('Name'),
	z.literal('sizeInBytes').describe('Size'),
	z.literal('dateModified').describe('Date Modified'),
	z.literal('dateIndexed').describe('Date Indexed'),
	z.literal('dateCreated').describe('Date Created'),
	z.literal('object.dateAccessed').describe('Date Accessed'),
	z.literal('object.mediaData.epochTime').describe('Date Taken')
]);

export const objectOrderingKeysSchema = z.union([
	z.literal('dateAccessed').describe('Date Accessed'),
	z.literal('kind').describe('Kind'),
	z.literal('mediaData.epochTime').describe('Date Taken')
]);

export const nonIndexedPathOrderingSchema = z.union([
	z.literal('name').describe('Name'),
	z.literal('sizeInBytes').describe('Size'),
	z.literal('dateCreated').describe('Date Created'),
	z.literal('dateModified').describe('Date Modified')
]);
