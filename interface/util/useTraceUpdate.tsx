import { useEffect, useRef } from 'react';

/**
 * DO NOT DELETE THIS HOOK
 * It probably isn't used in the codebase, but it's a useful debugging tool.
 */
export function useTraceUpdate(name: string, props: object | null) {
	const prev = useRef<{ [key: string]: any } | null>(props);
	useEffect(() => {
		const { current } = prev;
		if (props == null) {
			console.log(`Change ${name} to null`);
		} else if (current == null) {
			console.log(`Change ${name} from null to`, props);
		} else {
			const changedProps = Object.entries(props).reduce((ps: any, [k, v]) => {
				if (current[k] !== v) {
					ps[k] = [current[k], v];
				}
				return ps;
			}, {});
			if (Object.keys(changedProps).length > 0) {
				console.log(`Changed ${name}:`, changedProps);
			}
		}
		prev.current = props;
	});
}
