import {
	Link,
	RSPCError,
	Request as RspcRequest,
	Response as RspcResponse,
	_internal_fireResponse,
	_internal_wsLinkInternal
} from '@rspc/client';
// import { NativeEventEmitter, NativeModules } from 'react-native';
import { addChangeListener, coreStartupError } from '../../modules/sd-core';

// const { SDCore } = NativeModules;
// const eventEmitter = new NativeEventEmitter(NativeModules.SDCore);

// eslint-disable-next-line prefer-const
export let rspcSingletonContext = {
	id: null as number | null
};

export async function cleanupRspcContext(id: number) {
	// return SDCore.sd_cleanup_context(id);
}

/**
 * Link for the rspc Tauri plugin
 */
export function reactNativeLink(): Link {
	if (coreStartupError) {
		// TODO: Utku we are gonna wanna handle this with a proper screen.
		throw new Error('Failed to start Spacedrive core: ' + coreStartupError);
	}

	addChangeListener((e) => {
		console.log('RUST EVENT', e);
	});

	return _internal_wsLinkInternal(newWsManager());
}

function newWsManager() {
	const activeMap = new Map<
		number,
		{
			oneshot: boolean;
			resolve: (result: any) => void;
			reject: (error: Error | RSPCError) => void;
		}
	>();

	const handle = (resp: RspcResponse[] | RspcResponse) => {
		console.log(activeMap); // TODO

		const respArr = Array.isArray(resp) ? resp : [resp];
		for (const result of respArr) {
			const item = activeMap.get(result.id);

			if (!item) {
				console.error(`rspc: received event with id '${result.id}' for unknown`);
				return;
			}

			_internal_fireResponse(result, {
				resolve: item.resolve,
				reject: item.reject
			});
			if ((item.oneshot && result.type === 'value') || result.type === 'complete')
				activeMap.delete(result.id);
		}
	};

	// eventEmitter.addListener('SDCoreEvent', (event) => handle(JSON.parse(event)));

	return [
		activeMap,
		async (data: RspcRequest | RspcRequest[]) => {
			// console.log('ID AS', rspcSingletonContext.id); // TODO
			if (rspcSingletonContext.id === null)
				throw new Error('Something went wrong! rspc contextId is null');
			console.log('O');
			// const v = await SDCore.sd_core_msg(rspcSingletonContext.id, JSON.stringify(data));
			// console.log('V', v);
			// handle(JSON.parse(v));
		}
	] as const;
}
