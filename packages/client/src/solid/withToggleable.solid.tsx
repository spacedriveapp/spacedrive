/** @jsxImportSource solid-js */

import { createElement, FunctionComponent } from 'react';
import { createEffect, For } from 'solid-js';
import { createStore } from 'solid-js/store';

import { WithSolid } from './react';
import { useObserver } from './useObserver';

const [toggleableFeatures, setToggleableFeatures] = createStore({} as Record<string, boolean>);

export function ToggleablePanel() {
	return (
		<div class="w-100 absolute right-0 top-0 z-[99999] bg-red-500 p-2">
			<p>Toggles:</p>
			<For each={Object.entries(toggleableFeatures)}>
				{([name, obj]) => (
					<div class="flex flex items-center space-x-2">
						<input
							type="checkbox"
							checked={toggleableFeatures[name]}
							onChange={(e) =>
								setToggleableFeatures({ [name]: e.currentTarget.checked })
							}
						/>
						<span>{name}</span>
					</div>
				)}
			</For>
		</div>
	);
}

export function ToggleablePanelProvider() {
	return createElement(WithSolid, { root: ToggleablePanel }, null);
}

export function withToggleable<P>(name: string, a: FunctionComponent<P>, b: FunctionComponent<P>) {
	setToggleableFeatures({ [name]: false });

	return (props: P) => {
		const renderA = useObserver(() => toggleableFeatures[name] || false);
		return createElement((renderA ? b : a) as any, props, null);
	};
}
