/** @jsxImportSource solid-js */

import { trackDeep } from '@solid-primitives/deep';
import { createElement, StrictMode, type FunctionComponent } from 'react';
import ReactDOM from 'react-dom/client';
import { createEffect, onCleanup, splitProps } from 'solid-js';

type Props<T> =
	| {
			root: FunctionComponent<{}>;
	  }
	| ({
			root: FunctionComponent<T>;
	  } & T);

export function WithReact<T extends object>(props: Props<T>) {
	let ref: HTMLDivElement | undefined;
	let root: ReactDOM.Root | undefined;
	let cleanup: (() => void) | undefined = undefined;

	const [_, childProps] = splitProps(props, ['root']);

	// TODO: Inject all context's
	const render = (childProps: any) => {
		if (!ref) return;
		if (!root) {
			root = ReactDOM.createRoot(ref);
			// The `setTimeout` is to ensure React has time to do the intial render.
			// React doesn't like when you unmount it while it's rendering.
			cleanup = () => {
				setTimeout(() => root?.unmount());
				root = undefined;
			};
		}

		root.render(
			createElement(StrictMode, null, createElement(props.root as any, childProps, null))
		);
	};

	createEffect(() => {
		const trackedProps = trackDeep(childProps);
		render({ ...trackedProps });
	});

	onCleanup(() => {
		cleanup?.();
		cleanup = undefined;
	});

	return <div ref={ref} />;
}
