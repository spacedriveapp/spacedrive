/** No-op debug stub for CJS packages that import `debug` in the browser. */
function debug(_namespace: string) {
	return (..._args: unknown[]) => {};
}

debug.enable = () => {};
debug.disable = () => {};
debug.enabled = () => false;

export default debug;