import { ClientQuery, ClientResponse } from '@sd/core';
import { EventEmitter } from 'eventemitter3';

export let transport: BaseTransport | null = null;

export abstract class BaseTransport extends EventEmitter {
  abstract send(query: ClientQuery): Promise<unknown>;
}

export async function bridge<
  K extends ClientQuery['key'],
  CQ extends Extract<ClientQuery, { key: K }>
>(key: K, params?: CQ extends { params: any } ? CQ['params'] : never) {
  const result = (await transport?.send({ key, params } as any)) as any;
  console.log(`query: ${result?.key}`, result?.data);
  return result?.data;
}

export function setTransport(_transport: BaseTransport) {
  transport = _transport;
}
