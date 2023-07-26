import { requireNativeModule } from 'expo-modules-core';
import { EventEmitter, NativeModulesProxy, Subscription } from 'expo-modules-core';

const SDCoreModule = requireNativeModule('SDCore');

const emitter = new EventEmitter(SDCoreModule ?? NativeModulesProxy.SDCore);

export const coreStartupError: string | null = SDCoreModule.coreStartupError;

type SdCoreEvent = { data: string };

export function addChangeListener(listener: (event: SdCoreEvent) => void): Subscription {
	return emitter.addListener<SdCoreEvent>('sdCoreEvent', listener);
}
