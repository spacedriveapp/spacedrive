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
	createUniqueId,
	For,
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

type SolidPortal = {
	id: string;
	portal: JSX.Element;
};

export const solidPortalProvider = createReactContext<Setter<SolidPortal[]>>(undefined!);

export function WithReact<T extends object>(props: Props<T>) {
	const setReactPortals = useSolidContext(reactPortalProvider);
	if (!setReactPortals) throw new Error('No react portal provider context');

	const id = createUniqueId();
	const [portals, setPortals] = createSignal([] as SolidPortal[]);

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
		setReactPortals((portals) => [
			...portals,
			{
				id,
				portal
			}
		]);
	});

	onCleanup(() => {
		setReactPortals((portals) => portals.filter((p) => p.id !== id));
	});

	return (
		<>
			<div ref={ref} />
			<For each={portals()}>{(p) => <>{p.portal}</>}</For>
		</>
	);
}

function Wrapper<T extends object>(props: {
	root: FunctionComponent;
	owner: Owner;
	childProps: () => T;
}) {
	// This is a React component SolidJS reactivity don't matter.

	// eslint-disable-next-line solid/reactivity
	const childProps = useObserverWithOwner(props.owner, () => trackDeep(props.childProps()));
	// eslint-disable-next-line solid/reactivity
	return withReactContextProvider(props.owner, createElement(props.root, childProps, null));
}
