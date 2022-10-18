import {
	Operation,
	ProcedureType,
	ProceduresDef,
	TRPCClientOutgoingMessage,
	TRPCLink,
	TRPCRequestMessage,
	TRPCWebSocketClient,
	UnsubscribeFn,
	wsLink
} from '@rspc/client';
import { NativeEventEmitter, NativeModules } from 'react-native';

type TCallbacks = any; // TODO

const { SDCore } = NativeModules;
const eventEmitter = new NativeEventEmitter(NativeModules.SDCore);

export function reactNativeLink<TProcedures extends ProceduresDef>(): TRPCLink<TProcedures> {
	return wsLink<TProcedures>({
		client: createReactNativeClient()
	});
}

export function createReactNativeClient(): TRPCWebSocketClient {
	/**
	 * outgoing messages buffer whilst not open
	 */
	let outgoing: TRPCClientOutgoingMessage[] = [];
	/**
	 * pending outgoing requests that are awaiting callback
	 */
	type TRequest = {
		/**
		 * Reference to the WebSocket instance this request was made to
		 */
		ws: WebSocket;
		type: ProcedureType;
		callbacks: TCallbacks;
		op: Operation;
	};
	const pendingRequests: Record<number | string, TRequest> = Object.create(null);
	let dispatchTimer: ReturnType<typeof setTimeout> | number | null = null;
	let state: 'open' | 'closed' = 'open';

	function handleIncoming(data: any) {
		if ('method' in data) {
			//
		} else {
			const req = data.id !== null && pendingRequests[data.id];
			if (!req) {
				// do something?
				return;
			}
			req.callbacks.next?.(data);
			if ('result' in data && data.result.type === 'stopped') {
				req.callbacks.complete();
			}
		}
	}

	function dispatch() {
		if (state !== 'open' || dispatchTimer) {
			return;
		}
		dispatchTimer = setTimeout(() => {
			dispatchTimer = null;

			if (outgoing.length === 0) {
				return;
			}

			let body: any;
			if (outgoing.length === 1) {
				// single send
				body = JSON.stringify(outgoing.pop());
			} else {
				// batch send
				body = JSON.stringify(outgoing);
			}

			SDCore.sd_core_msg(body).then((rawData) => {
				const data = JSON.parse(rawData);
				if (Array.isArray(data)) {
					for (const payload of data) {
						handleIncoming(payload);
					}
				} else {
					handleIncoming(data);
				}
			});

			// clear
			outgoing = [];
		});
	}

	eventEmitter.addListener('SDCoreEvent', (event) => {
		const data = JSON.parse(event);
		handleIncoming(data);
	});

	function request(op: Operation, callbacks: TCallbacks): UnsubscribeFn {
		const { type, input, path, id } = op;
		const envelope: TRPCRequestMessage = {
			id,
			method: type,
			params: {
				input,
				path
			}
		};
		pendingRequests[id] = {
			ws: undefined as any, // TODO: Remove this field
			type,
			callbacks,
			op
		};
		// enqueue message
		outgoing.push(envelope);
		dispatch();
		return () => {
			const callbacks = pendingRequests[id]?.callbacks;
			delete pendingRequests[id];
			outgoing = outgoing.filter((msg) => msg.id !== id);
			callbacks?.complete?.();
			if (op.type === 'subscription') {
				outgoing.push({
					id,
					method: 'subscriptionStop'
				});
				dispatch();
			}
		};
	}

	return {
		close: () => {
			state = 'closed';
			// TODO: Close all open subscriptions
			//   closeIfNoPending(activeConnection);
			// TODO
		},
		request
	};
}
