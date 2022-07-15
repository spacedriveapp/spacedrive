import { ClientCommand, ClientQuery, CoreResponse, LibraryCommand, LibraryQuery } from '@sd/core';
import { EventEmitter } from 'eventemitter3';
import { UseMutationOptions, UseQueryOptions, useMutation, useQuery } from 'react-query';

import { useLibraryStore } from './stores';

// global var to store the transport TODO: not global :D
export let transport: BaseTransport | null = null;

// applications utilizing this package should extend this class to instantiate a transport
export abstract class BaseTransport extends EventEmitter {
	abstract query(query: ClientQuery): Promise<unknown>;
	abstract command(command: ClientCommand): Promise<unknown>;
}

export function setTransport(_transport: BaseTransport) {
	transport = _transport;
}

// extract keys from generated Rust query/command types
export type QueryKeyType = ClientQuery['key'];
export type LibraryQueryKeyType = LibraryQuery['key'];
export type CommandKeyType = ClientCommand['key'];
export type LibraryCommandKeyType = LibraryCommand['key'];

// extract the type from the union
type CQType<K> = Extract<ClientQuery, { key: K }>;
type LQType<K> = Extract<LibraryQuery, { key: K }>;
type CCType<K> = Extract<ClientCommand, { key: K }>;
type LCType<K> = Extract<LibraryCommand, { key: K }>;
type CRType<K> = Extract<CoreResponse, { key: K }>;

// extract payload type
type ExtractParams<P> = P extends { params: any } ? P['params'] : never;
type ExtractData<D> = D extends { data: any } ? D['data'] : never;

// vanilla method to call the transport
async function queryBridge<K extends QueryKeyType, CQ extends CQType<K>, CR extends CRType<K>>(
	key: K,
	params?: ExtractParams<CQ>
): Promise<ExtractData<CR>> {
	const result = (await transport?.query({ key, params } as any)) as any;
	return result?.data;
}

async function commandBridge<K extends CommandKeyType, CC extends CCType<K>, CR extends CRType<K>>(
	key: K,
	params?: ExtractParams<CC>
): Promise<ExtractData<CR>> {
	const result = (await transport?.command({ key, params } as any)) as any;
	return result?.data;
}

// react-query method to call the transport
export function useBridgeQuery<K extends QueryKeyType, CQ extends CQType<K>, CR extends CRType<K>>(
	key: K,
	params?: ExtractParams<CQ>,
	options: UseQueryOptions<ExtractData<CR>> = {}
) {
	return useQuery<ExtractData<CR>>(
		[key, params],
		async () => await queryBridge(key, params),
		options
	);
}

export function useLibraryQuery<
	K extends LibraryQueryKeyType,
	CQ extends LQType<K>,
	CR extends CRType<K>
>(key: K, params?: ExtractParams<CQ>, options: UseQueryOptions<ExtractData<CR>> = {}) {
	const library_id = useLibraryStore((state) => state.currentLibraryUuid);
	if (!library_id) throw new Error(`Attempted to do library query '${key}' with no library set!`);

	return useQuery<ExtractData<CR>>(
		[library_id, key, params],
		async () => await queryBridge('LibraryQuery', { library_id, query: { key, params } as any }),
		options
	);
}

export function useBridgeCommand<
	K extends CommandKeyType,
	CC extends CCType<K>,
	CR extends CRType<K>
>(key: K, options: UseMutationOptions<ExtractData<CC>> = {}) {
	return useMutation<ExtractData<CR>, unknown, ExtractParams<CC>>(
		[key],
		async (vars?: ExtractParams<CC>) => await commandBridge(key, vars),
		options
	);
}

export function useLibraryCommand<
	K extends LibraryCommandKeyType,
	LC extends LCType<K>,
	CR extends CRType<K>
>(key: K, options: UseMutationOptions<ExtractData<LC>> = {}) {
	const library_id = useLibraryStore((state) => state.currentLibraryUuid);
	if (!library_id) throw new Error(`Attempted to do library command '${key}' with no library set!`);

	return useMutation<ExtractData<CR>, unknown, ExtractParams<LC>>(
		[library_id, key],
		async (vars?: ExtractParams<LC>) =>
			await commandBridge('LibraryCommand', { library_id, command: { key, params: vars } as any }),
		options
	);
}

export function command<K extends CommandKeyType, CC extends CCType<K>, CR extends CRType<K>>(
	key: K,
	vars: ExtractParams<CC>
): Promise<ExtractData<CR>> {
	return commandBridge(key, vars);
}

export function libraryCommand<
	K extends LibraryCommandKeyType,
	LC extends LCType<K>,
	CR extends CRType<K>
>(key: K, vars: ExtractParams<LC>): Promise<ExtractData<CR>> {
	const library_id = useLibraryStore((state) => state.currentLibraryUuid);
	if (!library_id) throw new Error(`Attempted to do library command '${key}' with no library set!`);
	return commandBridge('LibraryCommand', { library_id, command: { key, params: vars } as any });
}
