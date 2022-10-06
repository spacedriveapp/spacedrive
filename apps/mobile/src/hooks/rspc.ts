import { OperationType, ProcedureDef, RSPCError, Transport } from '@rspc/client';
import { createReactQueryHooks } from '@rspc/react';
import { QueryClient } from '@tanstack/react-query';
import { NativeEventEmitter, NativeModules } from 'react-native';

import { getLibraryIdRaw } from '../stores/libraryStore';
import { LibraryArgs, Procedures } from '../types/bindings';

export const queryClient = new QueryClient();
export const rspc = createReactQueryHooks<Procedures>();

const { SDCore } = NativeModules;
const eventEmitter = new NativeEventEmitter(NativeModules.SDCore);

// TODO(@Oscar): Replace this with a better abstraction when it's released in rspc. This relies on internal details of rspc which will change without warning.
export class ReactNativeTransport implements Transport {
	clientSubscriptionCallback?: (id: string, value: any) => void;

	constructor() {
		const subscriptionEventListener = eventEmitter.addListener('SDCoreEvent', (event) => {
			const { id, result } = JSON.parse(event);
			if (result.type === 'event') {
				if (this.clientSubscriptionCallback) this.clientSubscriptionCallback(id, result.data);
			} else if (result.type === 'response' || result.type === 'error') {
				throw new Error(
					`Recieved event of type '${result.type}'. This should be impossible with the React Native transport!`
				);
			} else {
				console.error(`Received event of unknown method '${result.type}'`);
			}
		});
	}

	async doRequest(operation: OperationType, key: string, input: any): Promise<any> {
		const resp = JSON.parse(
			await SDCore.sd_core_msg(
				JSON.stringify({
					id: null,
					method: operation,
					params: {
						path: key,
						input
					}
				})
			)
		);

		const body = resp.result;
		if (body.type === 'error') {
			const { code, message } = body;
			throw new RSPCError(code, message);
		} else if (body.type === 'response') {
			return body.data;
		} else if (body.type !== 'none') {
			throw new Error(`RSPC ReactNative doRequest received invalid body type '${body?.type}'`);
		}
	}
}

type NonLibraryProcedure<T extends keyof Procedures> =
	| Exclude<Procedures[T], { input: LibraryArgs<any> }>
	| Extract<Procedures[T], { input: never }>;

type LibraryProcedures<T extends keyof Procedures> = Exclude<
	Extract<Procedures[T], { input: LibraryArgs<any> }>,
	{ input: never }
>;

type MoreConstrainedQueries<T extends ProcedureDef> = T extends any
	? T['input'] extends LibraryArgs<infer E>
		? {
				key: T['key'];
				input: E;
				result: T['result'];
		  }
		: never
	: never;

export const useBridgeQuery = rspc.customQuery<NonLibraryProcedure<'queries'>>(
	(keyAndInput) => keyAndInput as any
);

export const useBridgeMutation = rspc.customMutation<NonLibraryProcedure<'mutations'>>(
	(keyAndInput) => keyAndInput
);

export const useLibraryQuery = rspc.customQuery<
	MoreConstrainedQueries<LibraryProcedures<'queries'>>
>((keyAndInput) => {
	const library_id = getLibraryIdRaw();
	if (library_id === null) throw new Error('Attempted to do library query with no library set!');
	return [keyAndInput[0], { library_id, arg: keyAndInput[1] || null }];
});

export const useLibraryMutation = rspc.customMutation<
	MoreConstrainedQueries<LibraryProcedures<'mutations'>>
>((keyAndInput) => {
	const library_id = getLibraryIdRaw();
	if (library_id === null) throw new Error('Attempted to do library query with no library set!');
	return [keyAndInput[0], { library_id, arg: keyAndInput[1] || null }];
});

export function useInvalidateQuery() {
	const context = rspc.useContext();
	rspc.useSubscription(['invalidateQuery'], {
		onData: (invalidateOperation) => {
			const key = [invalidateOperation.key];
			if (invalidateOperation.arg !== null) {
				key.concat(invalidateOperation.arg);
			}
			context.queryClient.invalidateQueries(key);
		}
	});
}
