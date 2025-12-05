// @ts-ignore - Expo modules types may not be available in all environments
const { EventEmitter, NativeModulesProxy } = require("expo-modules-core");

const SDMobileCoreModule = NativeModulesProxy?.SDMobileCore;

if (!SDMobileCoreModule) {
	throw new Error(
		"SDMobileCore native module not found. Did you run 'cargo xtask build-mobile' and rebuild the app?",
	);
}

const emitter = new EventEmitter(SDMobileCoreModule);

export interface CoreEvent {
	body: string;
}

export interface CoreModule {
	initialize(dataDir?: string, deviceName?: string): Promise<number>;
	sendMessage(query: string): Promise<string>;
	shutdown(): void;
	addListener(callback: (event: CoreEvent) => void): () => void;
}

export const SDMobileCore: CoreModule = {
	initialize: async (dataDir?: string, deviceName?: string) => {
		return SDMobileCoreModule.initialize(
			dataDir ?? null,
			deviceName ?? null,
		);
	},
	sendMessage: async (query: string) => {
		return SDMobileCoreModule.sendMessage(query);
	},
	shutdown: () => {
		SDMobileCoreModule.shutdown();
	},
	addListener: (callback: (event: CoreEvent) => void) => {
		const subscription = emitter.addListener(
			"SDCoreEvent",
			callback as (event: CoreEvent) => void,
		);
		return () => subscription.remove();
	},
};
