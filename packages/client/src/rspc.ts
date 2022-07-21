import { createReactQueryHooks } from '@rspc/client';
import { QueryClient } from '@tanstack/react-query';

import type { Operations } from '../../../core/bindings/index';
import { useLibraryStore } from './stores';

export type { Operations } from '../../../core/bindings/index';

export const queryClient = new QueryClient();

export const rspc = createReactQueryHooks<Operations>();

// TODO: Their is no type safety if you were to call a library query in usBridgeQuery or vice-versa
// TODO: With the user deciding the keys we aren't gonna be able to do invalidate query well.

export const useBridgeQuery = rspc.customQuery((key, arg, options) => {
	return [
		[key, arg],
		arg,
		options,
		{
			library_id: undefined
		}
	];
});

export const useLibraryQuery = rspc.customQuery((key, arg, options) => {
	const library_id = useLibraryStore((state) => state.currentLibraryUuid);
	if (!library_id) throw new Error(`Attempted to do library query with no library set!`);

	return [
		[library_id, key, arg],
		arg,
		options,
		{
			library_id
		}
	];
});

export const useBridgeCommand = rspc.customMutation((key, arg, options) => {
	return [
		[key, arg],
		arg,
		options,
		{
			library_id: undefined
		}
	];
});

export const useLibraryCommand = rspc.customMutation((key, arg, options) => {
	const library_id = useLibraryStore((state) => state.currentLibraryUuid);
	if (!library_id) throw new Error(`Attempted to do library query with no library set!`);

	return [
		[library_id, key, arg],
		arg,
		options,
		{
			library_id
		}
	];
});

// TODO: Work out a solution for removing this
// @ts-ignore
export function libraryCommand<
	K extends LibraryCommandKeyType,
	LC extends LCType<K>,
	CR extends CRType<K>
>(key: K, vars: ExtractParams<LC>): Promise<ExtractData<CR>> {
	const library_id = useLibraryStore((state) => state.currentLibraryUuid);
	if (!library_id) throw new Error(`Attempted to do library command '${key}' with no library set!`);
	return commandBridge('LibraryCommand', { library_id, command: { key, params: vars } as any });
}
