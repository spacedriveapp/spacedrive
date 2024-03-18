/** @jsxImportSource solid-js */

import { trackDeep } from '@solid-primitives/deep';
import { createElement, StrictMode, type FunctionComponent } from 'react';
import { createPortal } from 'react-dom';
import {
	children,
	createSignal,
	createUniqueId,
	For,
	getOwner,
	onCleanup,
	onMount,
	Owner,
	splitProps,
	useContext as useSolidContext,
	type JSX as SolidJSX
} from 'solid-js';

import { withReactCtx as withReactContextProvider } from './context';
import { Portal, reactPortalCtx, solidPortalCtx } from './portals';
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

export function WithReact<T extends object>(props: Props<T>) {
	const portalCtx = useSolidContext(solidPortalCtx);
	if (!portalCtx) throw new Error('Missing portalCtx in WithReact');

	const [solidPortals, setSolidPortals] = createSignal([] as Portal<SolidJSX.Element>[]);
	const id = createUniqueId();
	let ref: HTMLDivElement | undefined;

	onMount(() => {
		if (!ref) return;

		const elem = createElement(
			StrictMode,
			null,
			createElement(
				reactPortalCtx.Provider,
				{
					value: {
						setReactPortals: portalCtx.setReactPortals,
						setSolidPortals: setSolidPortals
					}
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
		portalCtx.setReactPortals((portals) => [
			...portals,
			{
				id,
				portal
			}
		]);
	});

	onCleanup(() => portalCtx.setReactPortals((portals) => portals.filter((p) => p.id !== id)));

	return (
		<>
			<div ref={ref} />
			<For each={solidPortals()}>{(p) => children(() => p.portal) as any}</For>
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
