import { ClientQuery, ClientResponse } from '@sd/core';
import { EventEmitter } from 'eventemitter3';
import { useQuery } from 'react-query';

export let transport: BaseTransport | null = null;

export abstract class BaseTransport extends EventEmitter {
  abstract send(query: ClientQuery): Promise<unknown>;
}

export async function bridge<
  K extends ClientQuery['key'],
  CQ extends Extract<ClientQuery, { key: K }>
>(key: K, params?: CQ extends { params: any } ? CQ['params'] : never) {
  const result = (await transport?.send({ key, params } as any)) as any;
  console.log(`ClientQueryTransport: [${result?.key}]`, result?.data);
  return result?.data;
}

export function setTransport(_transport: BaseTransport) {
  transport = _transport;
}

export function useBridgeQuery(
  key: Parameters<typeof bridge>[0],
  params?: Parameters<typeof bridge>[1],
  options: Parameters<typeof useQuery>[2] = {}
) {
  return useQuery([key, params], () => bridge(key, params), options);
}
