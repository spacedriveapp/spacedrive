import {
  NativeModule,
  requireNativeModule,
  EventEmitter,
} from "expo-modules-core";

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

type SDMobileCoreEvents = {
  SDCoreEvent: (event: CoreEvent) => void;
  SDCoreLog: (log: CoreLog) => void;
};

export interface FolderPickerResult {
  uri: string;
  path: string | null;
  name: string;
}

export interface CoreModule {
  initialize(dataDir?: string, deviceName?: string): Promise<number>;
  sendMessage(query: string): Promise<string>;
  shutdown(): void;
  addListener(callback: (event: CoreEvent) => void): () => void;
  addLogListener(callback: (log: CoreLog) => void): () => void;
  pickFolder(): Promise<FolderPickerResult>;
  getPathFromUri(uri: string): string | null;
  /** Check if the app has full storage access permission (Android 11+) */
  hasStoragePermission(): boolean;
  /** Check if full storage permission is required for this Android version */
  requiresStoragePermission(): boolean;
  /** Open system settings to grant "All Files Access" permission */
  openStoragePermissionSettings(): boolean;
}

interface SDMobileCoreNativeModule extends NativeModule<SDMobileCoreEvents> {
  initialize(
    dataDir: string | null,
    deviceName: string | null,
  ): Promise<number>;
  sendMessage(query: string): Promise<string>;
  shutdown(): void;
  addListener(callback: (event: CoreEvent) => void): () => void;
  addLogListener(callback: (log: CoreLog) => void): () => void;
  pickFolder(options: Record<string, unknown>): Promise<FolderPickerResult>;
  getPathFromUri(uri: string): string | null;
  hasStoragePermission(): boolean;
  requiresStoragePermission(): boolean;
  openStoragePermissionSettings(): boolean;
}

const SDMobileCoreModule =
  requireNativeModule<SDMobileCoreNativeModule>("SDMobileCore");

if (!SDMobileCoreModule) {
  throw new Error("SDMobileCoreModule has not been initialized. Did you run 'cargo xtask build-mobile' and rebuild the app?")
}

const emitter = new EventEmitter<SDMobileCoreEvents>(SDMobileCoreModule as any);

export const SDMobileCore: CoreModule = {
  initialize: async (dataDir?: string, deviceName?: string) => {
    return SDMobileCoreModule.initialize(dataDir ?? null, deviceName ?? null);
  },
  sendMessage: async (query: string) => {
    return SDMobileCoreModule.sendMessage(query);
  },
  shutdown: () => {
    SDMobileCoreModule.shutdown();
  },
  addListener: (callback: (event: CoreEvent) => void) => {
    const subscription = emitter.addListener("SDCoreEvent", callback);
    return () => subscription.remove();
  },
  addLogListener: (callback: (log: CoreLog) => void) => {
    const subscription = emitter.addListener("SDCoreLog", callback);
    return () => subscription.remove();
  },
  pickFolder: async () => {
    return SDMobileCoreModule.pickFolder({});
  },
  getPathFromUri: (uri: string) => {
    return SDMobileCoreModule.getPathFromUri(uri);
  },
  hasStoragePermission: () => {
    return SDMobileCoreModule.hasStoragePermission();
  },
  requiresStoragePermission: () => {
    return SDMobileCoreModule.requiresStoragePermission();
  },
  openStoragePermissionSettings: () => {
    return SDMobileCoreModule.openStoragePermissionSettings();
  },
};
