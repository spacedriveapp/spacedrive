import {
	Link,
	RSPCError,
	Request as RspcRequest,
	Response as RspcResponse,
	_internal_fireResponse,
	_internal_wsLinkInternal
} from '@rspc/client';
import { NativeEventEmitter, NativeModules } from 'react-native';

const { SDCore } = NativeModules;
const eventEmitter = new NativeEventEmitter(NativeModules.SDCore);

/**
 * Link for the rspc Tauri plugin
 */
export function reactNativeLink(): Link {
	return _internal_wsLinkInternal(newWsManager());
}

function newWsManager() {
	const activeMap = new Map<
		number,
		{
			resolve: (result: any) => void;
			reject: (error: Error | RSPCError) => void;
		}
	>();

	const handle = (resp: RspcResponse[] | RspcResponse) => {
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
			if (result.type === 'value' || result.type === 'complete') activeMap.delete(result.id);
		}
	};

	eventEmitter.addListener('SDCoreEvent', (event) => handle(JSON.parse(event)));

	return [
		activeMap,
		async (data: RspcRequest | RspcRequest[]) =>
			handle(JSON.parse(await SDCore.sd_core_msg(JSON.stringify(data))))
	] as const;
}
