import { ProcedureDef } from '@rspc/client';

// https://stackoverflow.com/a/54487392
export type OmitDistributive<T, K extends PropertyKey> = T extends any
	? T extends object
		? Id<OmitRecursively<T, K>>
		: T
	: never;
export type Id<T> = {} & { [P in keyof T]: T[P] }; // Cosmetic use only makes the tooltips expand the type can be removed
export type OmitRecursively<T extends any, K extends PropertyKey> = Omit<
	{ [P in keyof T]: OmitDistributive<T[P], K> },
	K
>;

/**
 * is responsible for normalizing the Typescript type before the type is exposed back to the user.
 *
 * @internal
 */
export type Normalized<T extends ProcedureDef> = T extends any
	? {
			key: T['key'];
			// TODO: Typescript transformation for arrays
			result: OmitRecursively<T['result'], '$id' | '$type'>;
			input: T['input'];
	  }
	: never;
