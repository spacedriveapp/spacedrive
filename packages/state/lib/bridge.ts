import { ClientQuery, CoreResponse } from '@sd/core';
import { EventEmitter } from 'eventemitter3';
import { useQuery, UseQueryOptions, UseQueryResult } from 'react-query';

export let transport: BaseTransport | null = null;

export abstract class BaseTransport extends EventEmitter {
  abstract send(query: ClientQuery): Promise<unknown>;
}

type KeyType = ClientQuery['key'];
type CQType<K> = Extract<ClientQuery, { key: K }>;
type CRType<K> = Extract<CoreResponse, { key: K }>;

type CQParams<CQ> = CQ extends { params: any } ? CQ['params'] : never;
type CRData<CR> = CR extends { data: any } ? CR['data'] : never;

export async function bridge<K extends KeyType, CQ extends CQType<K>, CR extends CRType<K>>(
  key: K,
  params?: CQParams<CQ>
): Promise<CRData<CR>> {
  const result = (await transport?.send({ key, params } as any)) as any;
  // console.log(`Client Query Transport: [${result?.key}]`, result?.data);
  return result?.data;
}

export function setTransport(_transport: BaseTransport) {
  transport = _transport;
}

export function useBridgeQuery<K extends KeyType, CQ extends CQType<K>, CR extends CRType<K>>(
  key: K,
  params?: CQParams<CQ>,
  options: UseQueryOptions<CRData<CR>> = {}
) {
  return useQuery<CRData<CR>>([key, params], async () => await bridge(key, params), options);
}
