import {
	ReactPortal,
	useEffect,
	useId,
	useContext as useReactContext,
	useRef,
	useState
} from 'react';
import { createContext as createSolidContext, JSX as SolidJSX } from 'solid-js';
import { createStore } from 'solid-js/store';
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

type Portal = {
	id: string;
	portal: ReactPortal;
};

export const reactPortalProvider = createSolidContext<
	(cb: (portals: Portal[]) => Portal[]) => void
>(undefined!);

export function WithSolid<T>(props: Props<T>) {
	const id = useId();
	const setReactPortals = useReactContext(solidPortalProvider);
	const ref = useRef<HTMLDivElement>(null);
	const state = useRef({
		hasFirstRender: false
	});
	const [portals, setPortals] = useState([] as Portal[]);

	const applyCtx = useWithContextReact();
	const trackedProps = useRef(createStore(props));

	useEffect(() => {
		trackedProps.current[1](props);
	}, [props]);

	useEffect(() => {
		if (!ref.current) return;

		const hasFirstRender = state.current.hasFirstRender;
		if (!hasFirstRender) {
			state.current.hasFirstRender = true;
			return;
		}

		if (setReactPortals) {
			// We are within a `UseSolid` so we should use it's React root.

			setReactPortals((portals) => {
				return [
					...portals,
					{
						id,
						portal: Portal({
							mount: ref.current!,
							get children() {
								return props.root(trackedProps.current[0] as T);
							}
						}) as any
					}
				];
			});
		} else {
			// We are not within a `UseSolid` so we need to setup the root.

			// TODO: Do we need to setup a nested context

			let cleanup = () => {};
			if (ref.current)
				cleanup = render(() => {
					return applyCtx(() =>
						reactPortalProvider.Provider({
							value: setPortals,
							get children() {
								return props.root(trackedProps.current[0] as T);
							}
						})
					);
				}, ref.current);
			return cleanup;
		}

		return () => {
			if (!hasFirstRender) return;
			setReactPortals((portals) => portals.filter((p) => p.id !== id));
		};
		// This rerunning is super expensive so we wanna avoid it at all costs
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [props.root]);

	return (
		<>
			<div ref={ref} />
			{portals.map((p) => p.portal)}
		</>
	);
}
