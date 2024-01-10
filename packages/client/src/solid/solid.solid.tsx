/** @jsxImportSource solid-js */

import { trackDeep } from '@solid-primitives/deep';
import {
	createElement,
	createContext as createReactContext,
	StrictMode,
	type FunctionComponent
} from 'react';
import { createPortal } from 'react-dom';
import {
	createEffect,
	createSignal,
	getOwner,
	onCleanup,
	Owner,
	splitProps,
	useContext as useSolidContext,
	type Setter
} from 'solid-js';

import { withReactCtx as withReactContextProvider } from './context';
import { reactPortalProvider } from './react';
import { useObserverWithOwner } from './useObserver';

type AllowReactiveScope<T> = T extends object
	? {
			[P in keyof T]: AllowReactiveScope<T[P]>;
	  }
	: T | (() => T);

type Props<T> =
	| {
			root: FunctionComponent<{}>;
	  }
	| ({
			root: FunctionComponent<T>;
	  } & AllowReactiveScope<T>);

export const solidPortalProvider = createReactContext<Setter<JSX.Element[]>>(undefined!);

export function WithReact<T extends object>(props: Props<T>) {
	const reactPortalCtx = useSolidContext(reactPortalProvider);
	if (!reactPortalCtx) throw new Error('No react portal provider context');

	const [portals, setPortals] = createSignal([] as JSX.Element[]);

	let ref: HTMLDivElement | undefined;

	createEffect(() => {
		if (!ref) return;

		const elem = createElement(
			StrictMode,
			null,
			createElement(
				solidPortalProvider.Provider,
				{
					value: setPortals
				},
				createElement(
					Wrapper,
					{
						root: props.root as any,
						owner: getOwner()!,
						childProps: () => splitProps(props, ['root'])[1]
					},
					null
				)
			)
		);

		const portal = createPortal(elem, ref);
		reactPortalCtx((portals) => [...portals, portal]);
	});

	onCleanup(() => {
		// TODO: Properly cleanup portal
	});

	return (
		<>
			<div ref={ref} />
			{portals()}
		</>
	);
}

function Wrapper<T extends object>(props: {
	root: FunctionComponent;
	owner: Owner;
	childProps: () => T;
}) {
	const childProps = useObserverWithOwner(props.owner, () => trackDeep(props.childProps()));
	return withReactContextProvider(props.owner, createElement(props.root, childProps, null));
}
