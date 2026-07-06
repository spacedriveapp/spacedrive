// ESM shim for the CJS `debug` package (used by react-markdown / micromark chain).

type DebugFn = ((...args: unknown[]) => void) & {
	enabled: boolean;
	namespace: string;
};

function createDebug(namespace: string): DebugFn {
	const log = ((...args: unknown[]) => {
		if (log.enabled) console.debug(`[${namespace}]`, ...args);
	}) as DebugFn;
	log.enabled = false;
	log.namespace = namespace;
	return log;
}

createDebug.enable = (_namespaces?: string) => {};
createDebug.disable = () => {};
createDebug.enabled = (_namespace: string) => false;
createDebug.names = [] as string[];
createDebug.skips = [] as string[];
createDebug.formatters = {} as Record<string, unknown>;

export default createDebug;
