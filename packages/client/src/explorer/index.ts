import { SortOrder } from '../core';

export * from './useExplorerInfiniteQuery';
export * from './usePathsInfiniteQuery';
export * from './usePathsOffsetInfiniteQuery';
export * from './usePathsExplorerQuery';
export * from './useObjectsInfiniteQuery';
export * from './useObjectsOffsetInfiniteQuery';
export * from './useObjectsExplorerQuery';

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

export function orderingKey(ordering: Ordering): OrderingKey {
	let base = ordering.field;

	if (typeof ordering.value === 'object') {
		base += `.${orderingKey(ordering.value)}`;
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
