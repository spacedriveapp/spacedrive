import { ClientCommand, ClientQuery, CoreResponse } from '@sd/core';
import { EventEmitter } from 'eventemitter3';
import {
  useMutation,
  useQuery,
  UseQueryOptions,
  UseQueryResult,
  UseMutationOptions
} from 'react-query';

// global var to store the transport
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
type QueryKeyType = ClientQuery['key'];
type CommandKeyType = ClientCommand['key'];

// extract the type from the union
type CQType<K> = Extract<ClientQuery, { key: K }>;
type CCType<K> = Extract<ClientCommand, { key: K }>;
type CRType<K> = Extract<CoreResponse, { key: K }>;

// extract payload type
type ExtractParams<P> = P extends { params: any } ? P['params'] : never;
type ExtractData<D> = D extends { data: any } ? D['data'] : never;

// vanilla method to call the transport
export async function queryBridge<
  K extends QueryKeyType,
  CQ extends CQType<K>,
  CR extends CRType<K>
>(key: K, params?: ExtractParams<CQ>): Promise<ExtractData<CR>> {
  const result = (await transport?.query({ key, params } as any)) as any;
  return result?.data;
}

export async function commandBridge<
  K extends CommandKeyType,
  CC extends CCType<K>,
  CR extends CRType<K>
>(key: K, params?: ExtractParams<CC>): Promise<ExtractData<CR>> {
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

export function useBridgeCommand<
  K extends CommandKeyType,
  CC extends CCType<K>,
  CR extends CRType<K>
>(key: K, params?: ExtractParams<CC>, options: UseMutationOptions<ExtractData<CC>> = {}) {
  return useMutation<ExtractData<CR>>(
    [key, params],
    async () => await commandBridge(key, params),
    options
  );
}
