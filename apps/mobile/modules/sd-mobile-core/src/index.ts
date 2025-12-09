// TODO: Test if we can rely on Expo's autolinking instead of manually requiring the module
import { requireNativeModule, EventEmitter } from "expo-modules-core";

const SDMobileCoreModule = requireNativeModule("SDMobileCore");

const emitter = new EventEmitter(SDMobileCoreModule);

export interface CoreEvent {
	body: string;
}

export interface LogMessage {
	timestamp: string;
	level: string;
	target: string;
	message: string;
	job_id?: string;
	library_id?: string;
}

export interface CoreLog {
	body: string;
}

export interface CoreModule {
	initialize(dataDir?: string, deviceName?: string): Promise<number>;
	sendMessage(query: string): Promise<string>;
	shutdown(): void;
	addListener(callback: (event: CoreEvent) => void): () => void;
	addLogListener(callback: (log: CoreLog) => void): () => void;
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
	addLogListener: (callback: (log: CoreLog) => void) => {
		const subscription = emitter.addListener(
			"SDCoreLog",
			callback as (log: CoreLog) => void,
		);
		return () => subscription.remove();
	},
};
