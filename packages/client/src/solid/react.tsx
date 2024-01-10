import { ReactPortal, useEffect, useContext as useReactContext, useRef, useState } from 'react';
import { createContext as createSolidContext, JSX as SolidJSX } from 'solid-js';
import { createMutable, createStore } from 'solid-js/store';
import { Portal, render } from 'solid-js/web';

import { useWithContextReact } from './context';
import { solidPortalProvider } from './solid.solid';

type Props<T> =
	| ({
			root: (props: T) => SolidJSX.Element;
	  } & T)
	| {
			root: () => SolidJSX.Element;
	  };

export const reactPortalProvider = createSolidContext<
	(cb: (portals: ReactPortal[]) => ReactPortal[]) => void
>(undefined!);

export function WithSolid<T>(props: Props<T>) {
	const reactPortalCtx = useReactContext(solidPortalProvider);
	// if (!reactPortalCtx) throw new Error('No solid portal provider context'); // TODO: Enable this

	const ref = useRef<HTMLDivElement>(null);
	const state = useRef({
		hasFirstRender: false
	});
	const [portals, setPortals] = useState<ReactPortal[]>([]);

	const applyCtx = useWithContextReact();
	const trackedProps = useRef(createStore(props));

	useEffect(() => {
		console.log('PROPS CHANGED', JSON.stringify(props));
		trackedProps.current[1](props);
	}, [props]);

	useEffect(() => {
		if (!ref.current) return;

		if (!state.current.hasFirstRender) {
			state.current.hasFirstRender = true;
			return;
		}

		if (reactPortalCtx) {
			// We are within a `UseSolid` so we should use it's React root.

			reactPortalCtx((portals) => {
				return [
					...portals,
					Portal({
						mount: ref.current!,
						get children() {
							return props.root(trackedProps.current[0] as T);
						}
					})
				];
			});
		} else {
			// We are not within a `UseSolid` so we need to setup the root.

			// TODO: Do we need to setup a nested context

			let cleanup = () => {};
			if (ref.current)
				cleanup = render(() => {
					const { root, ...childProps } = props;
					return applyCtx(() =>
						reactPortalProvider.Provider({
							value: setPortals,
							get children() {
								return root(trackedProps.current[0] as T);
							}
						})
					);
				}, ref.current);
			return cleanup;
		}

		return () => {
			if (!state.current.hasFirstRender) return;

			// TODO: Cleanup `Portal`.
		};

		// console.log('GOT', reactPortalCtx);

		// if (reactPortalCtx) {
		// 	reactPortalCtx((portals) => {
		// 		return [
		// 			...portals,
		// 			Portal({
		// 				mount: ref.current!,
		// 				get children() {
		// 					return props.root(props as any);
		// 					// return 'CHILD';
		// 				}
		// 			})
		// 		];
		// 	});

		// 	// TODO: Cleanup portal
		// } else {
		// 	// TODO: Remove this condition in the future.

		// 	let cleanup = () => {};
		// 	if (ref.current)
		// 		cleanup = render(() => {
		// 			const { root, ...childProps } = props;
		// 			return applyCtx(() =>
		// 				reactPortalProvider.Provider({
		// 					value: setPortals,
		// 					get children() {
		// 						return root(childProps as any);
		// 					}
		// 				})
		// 			);
		// 		}, ref.current);
		// 	return cleanup;

		// 	// TODO: We are at the top and need to setup the context.
		// 	// createElement(
		// 	// 	solidPortalProvider.Provider,
		// 	// 	{
		// 	// 		value: setPortals
		// 	// 	},
		// 	// }
		// }
	}, []);

	return (
		<>
			<div ref={ref} />
			{portals}
		</>
	);
}
