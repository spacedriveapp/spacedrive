import {
	createElement,
	createContext as createReactContext,
	PropsWithChildren,
	ReactPortal,
	useRef,
	useState
} from 'react';
import {
	createSignal,
	createContext as createSolidContext,
	getOwner,
	Owner,
	ParentProps,
	Setter,
	type Context as SolidContext
} from 'solid-js';

import { useObserver } from './useObserver';

type PortalCtx = {
	reactPortals: Setter<ReactPortal>[];
	solidPortals: Setter<ReactPortal[]>;
};

type Portal<T> = {
	id: string;
	portal: T;
};

// The Solid provider is the source of truth.
// The React provider is just used to hook into the Solid provider.
export const solidProvider = createSolidContext(undefined! as PortalCtx);
export const reactProvider = createReactContext(undefined! as Owner);

// TODO: Right now we have React as our app's root so we don't need this
// export function InteropProviderSolid(props: ParentProps) {}

export function InteropProviderReact(props: PropsWithChildren) {
	const solidPortals = useRef(createSignal([] as Portal<ReactPortal>[]));
	const reactPortals = useRef(createSignal([] as Portal<ReactPortal>[]));
	const solidRoot = useObserver(() => {
		return {
			solidOwner: getOwner()!
			// reactPortals: reactPortals.current[0](),
			// solidPortals: solidPortals.current[0]()
		};
	});

	// return portalProvider.Provider({

	// TODO: Wrap with solid context provider
	return createElement(
		reactProvider.Provider,
		{
			value: solidRoot.solidOwner
		},
		props.children
	);
}
