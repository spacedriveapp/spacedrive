import { Transport } from '@rspc/client';

declare global {
	// eslint-disable-next-line
	var isDev: boolean;
	// eslint-disable-next-line
	var rspcTransport: Transport;
}

if (
	globalThis.localStorage === undefined ||
	globalThis.isDev === undefined ||
	globalThis.rspcTransport === undefined
)
	throw new Error('Please ensure you have patched `globalThis` before importing `@sd/client`!');

export * from './hooks';
export * from './stores';
export * from './rspc';
export * from './core';
export * from './utils';
export * from './lib';
