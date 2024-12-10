import { Range, SearchFilterArgs } from '@sd/client';

// Type guard to check if arg contains the 'filePath' with the appropriate field.
function isFilePathWithRange(
	arg: SearchFilterArgs,
	field: 'createdAt' | 'modifiedAt' | 'indexedAt'
): arg is { filePath: { [key in typeof field]: Range<string> } } {
	return 'filePath' in arg && typeof arg.filePath === 'object' && field in arg.filePath;
}

// Type guard to check if arg contains the 'object' with the appropriate field.
function isObjectWithRange(
	arg: SearchFilterArgs,
	field: 'dateAccessed'
): arg is { object: { [key in typeof field]: Range<string> } } {
	return 'object' in arg && typeof arg.object === 'object' && field in arg.object;
}

/**
 * Extracts a range (from and to) from the filePath part of SearchFilterArgs.
 * Handles fields like 'createdAt', 'modifiedAt', and 'indexedAt'.
 *
 * @param arg The search filter arguments.
 * @param field The specific range field to extract.
 * @returns A Range<string> object with from and to values, or undefined if not found.
 */
export function extractFilePathRange(
	arg: SearchFilterArgs,
	field: 'createdAt' | 'modifiedAt' | 'indexedAt'
): Range<string> | undefined {
	if (isFilePathWithRange(arg, field)) {
		const range = arg.filePath[field];

		// Handle cases where only `from` or `to` exists
		const from = 'from' in range ? range.from : '';
		const to = 'to' in range ? range.to : '';

		return { from, to };
	}
	return undefined;
}

/**
 * Extracts a range (from and to) from the object part of SearchFilterArgs.
 * Handles the 'dateAccessed' field.
 *
 * @param arg The search filter arguments.
 * @param field The specific range field to extract.
 * @returns A Range<string> object with from and to values, or undefined if not found.
 */
export function extractObjectRange(
	arg: SearchFilterArgs,
	field: 'dateAccessed'
): Range<string> | undefined {
	if (isObjectWithRange(arg, field)) {
		const range = arg.object[field];

		// Handle cases where only `from` or `to` exists
		const from = 'from' in range ? range.from : '';
		const to = 'to' in range ? range.to : '';

		return { from, to };
	}
	return undefined;
}

// Utility type that omits common properties from the filter
export type OmitCommonFilterProperties<T> = Omit<
	T,
	| 'conditions'
	| 'getCondition'
	| 'argsToFilterOptions'
	| 'setCondition'
	| 'applyAdd'
	| 'applyRemove'
	| 'create'
	| 'merge'
>;
