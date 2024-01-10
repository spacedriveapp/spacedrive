import {
	createElement,
	Fragment,
	ReactPortal,
	useEffect,
	useId,
	useContext as useReactContext,
	useRef
} from 'react';
import { Accessor, createSignal, JSX as SolidJSX } from 'solid-js';
import { createStore } from 'solid-js/store';
import { Portal as SolidPortal } from 'solid-js/web';

import { useWithContextReact } from './context';
import { reactPortalCtx, solidPortalCtx, type Portal } from './portals';
import { useObserver } from './useObserver';

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
		hasFirstRender: false,
		trackedProps: createStore(props),
		reactPortals: createSignal([] as Portal<ReactPortal>[])
	});

	const applyCtx = useWithContextReact();

	useEffect(() => {
		state.current.trackedProps[1](props);
	}, [props]);

	useEffect(() => {
		if (!ref.current) return;

		const hasFirstRender = state.current.hasFirstRender;
		if (!hasFirstRender) {
			state.current.hasFirstRender = true;
			return;
		}

		portalCtx.setSolidPortals((portals) => [
			...portals,
			{
				id,
				portal: (() => {
					return SolidPortal({
						mount: ref.current!,
						get children() {
							return solidPortalCtx.Provider({
								value: {
									setSolidPortals: portalCtx.setSolidPortals,
									setReactPortals: state.current.reactPortals[1]
								},
								get children() {
									return applyCtx(() =>
										props.root(state.current.trackedProps[0] as T)
									);
								}
							});
						}
					});
				}) as any
			}
		]);

		return () => {
			if (!hasFirstRender) return;
			portalCtx.setSolidPortals((portals) => portals.filter((p) => p.id !== id));
		};
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, []); // TODO: props.root

	return (
		<>
			<div ref={ref} />
			<RenderPortals portals={state.current.reactPortals[0]} />
		</>
	);
}

function RenderPortals(props: { portals: Accessor<Portal<ReactPortal>[]> }) {
	const portals = useObserver(() => props.portals());
	return portals.map((p) => createElement(Fragment, { key: p.id }, p.portal));
}
