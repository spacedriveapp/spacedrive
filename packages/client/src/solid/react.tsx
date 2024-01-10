import { useEffect, useId, useContext as useReactContext, useRef } from 'react';
import { JSX as SolidJSX } from 'solid-js';
import { createStore } from 'solid-js/store';
import { Portal } from 'solid-js/web';

import { reactPortalCtx, solidPortalCtx } from './portals';

type Props<T> =
	| ({
			root: (props: T) => SolidJSX.Element;
	  } & T)
	| {
			root: () => SolidJSX.Element;
	  };

export function WithSolid<T>(props: Props<T>) {
	const portalCtx = useReactContext(reactPortalCtx);
	if (!portalCtx) throw new Error('Missing portalCtx in WithSolid');

	const id = useId();
	const ref = useRef<HTMLDivElement>(null);
	const state = useRef({
		hasFirstRender: false
	});

	// const applyCtx = useWithContextReact(); // TODO: Make this work
	const trackedProps = useRef(createStore(props));

	// TODO
	// useEffect(() => {
	// 	console.log('PROPS CHANGE');
	// 	trackedProps.current[1](props);
	// }, [props]);

	useEffect(() => {
		if (!ref.current) return;

		const hasFirstRender = state.current.hasFirstRender;
		if (!hasFirstRender) {
			state.current.hasFirstRender = true;
			return;
		}

		console.log('RENDER SOLID FIRED', id);

		portalCtx.setSolidPortals((portals) => [
			...portals,
			{
				id,
				portal: (() =>
					Portal({
						mount: ref.current!,
						get children() {
							return props.root(trackedProps.current[0] as T);
							// return solidPortalCtx.Provider({
							// 	value: portalCtx,
							// 	get children() {
							// 		// TODO: Shared context providers???
							// 		return props.root(trackedProps.current[0] as T);
							// 	}
							// });
						}
					})) as any
			}
		]);

		return () => {
			if (!hasFirstRender) return;
			portalCtx.setSolidPortals((portals) => portals.filter((p) => p.id !== id));
		};
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, []); // TODO: props.root

	return <div ref={ref} />;
}
