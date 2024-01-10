import {
	createElement,
	createContext as createReactContext,
	Fragment,
	PropsWithChildren,
	ReactPortal,
	useEffect,
	useRef
} from 'react';
import {
	createSignal,
	createContext as createSolidContext,
	Setter,
	JSX as SolidJSX,
	type Accessor
} from 'solid-js';
import { render } from 'solid-js/web';

import { useObserver } from './useObserver';

export type PortalCtx = {
	setSolidPortals: Setter<Portal<SolidJSX.Element>[]>;
	setReactPortals: Setter<Portal<ReactPortal>[]>;
};

export type Portal<T> = {
	id: string;
	portal: T;
};

export const solidPortalCtx = createSolidContext(undefined! as PortalCtx);
export const reactPortalCtx = createReactContext(undefined! as PortalCtx);

// This component must exist above all `WithSolid` calls at the root of your React application.
// This component setups up the SolidJS root and portal providers so that all `WithSolid` and `WithReact` can render into the same React/Solid root.
export function InteropProviderReact(props: PropsWithChildren) {
	const state = useRef({
		solidPortals: createSignal([] as Portal<SolidJSX.Element>[]),
		reactPortals: createSignal([] as Portal<ReactPortal>[]),
		// We only render portals in this so it's never rendered to the DOM
		solidRoot: document.createElement('div'),
		didFireFirstRender: false
	});

	useEffect(() => {
		// This is to avoid double-rendering SolidJS when used in `React.StrictMode`.
		if (!state.current.didFireFirstRender) {
			state.current.didFireFirstRender = true;
			return;
		}

		let cleanup = () => {};
		cleanup = render(() => {
			return (() => state.current.solidPortals[0]().map((p) => p.portal)) as any;
		}, state.current.solidRoot);
		return cleanup;
	}, []);

	const value: PortalCtx = {
		setSolidPortals: state.current.solidPortals[1],
		setReactPortals: state.current.reactPortals[1]
	};
	const portals = createElement(RenderPortals, { portals: state.current.reactPortals[0] });
	return createElement(
		reactPortalCtx.Provider,
		{
			value
		},
		props.children,
		portals
	);
}

function RenderPortals(props: { portals: Accessor<Portal<ReactPortal>[]> }) {
	const portals = useObserver(() => props.portals());
	return portals.map((p) => createElement(Fragment, { key: p.id }, p.portal));
}

// TODO: Right now we have React as our app's root so we don't need this but it would be the opposite of `InteropProviderReact`
// export function InteropProviderSolid(props: ParentProps) {}
