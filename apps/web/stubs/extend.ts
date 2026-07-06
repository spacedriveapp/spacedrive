// ESM default export shim for CJS `extend` (object merge; used by markdown/unified stack).

type ExtendTarget = Record<string, unknown>;

function isPlainObject(value: unknown): value is ExtendTarget {
	return (
		value !== null &&
		typeof value === "object" &&
		Object.prototype.toString.call(value) === "[object Object]"
	);
}

function extend(
	deep: boolean | ExtendTarget,
	target?: ExtendTarget,
	...sources: ExtendTarget[]
): ExtendTarget {
	let useDeep = false;
	let args: ExtendTarget[];

	if (typeof deep === "boolean") {
		useDeep = deep;
		args = [target ?? {}, ...sources].filter(Boolean) as ExtendTarget[];
	} else {
		args = [deep, target, ...sources].filter(Boolean) as ExtendTarget[];
	}

	const out: ExtendTarget = { ...args[0] };
	for (const src of args.slice(1)) {
		for (const [key, value] of Object.entries(src)) {
			if (
				useDeep &&
				isPlainObject(out[key]) &&
				isPlainObject(value)
			) {
				out[key] = extend(true, out[key] as ExtendTarget, value);
			} else if (value !== undefined) {
				out[key] = value;
			}
		}
	}
	return out;
}

export default extend;
