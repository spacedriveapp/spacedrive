/** @jsxImportSource solid-js */

import { createSignal } from 'solid-js';
import { render } from 'solid-js/web';

import { Demo2 } from './demo2.solid';

function Demo() {
	const [count, setCount] = createSignal(0);

	return (
		<div class="absolute top-0 z-[99999] bg-red-500">
			<button onClick={() => setCount(count() + 1)}>Click me</button>
			<div>Hello from Solid: {count()}</div>
			<Demo2 />
		</div>
	);
}

// TODO: Get eslint error working for destructuring
function TestEslint({ demo }: { demo: string }) {}

export function renderDemo(element: HTMLDivElement): () => void {
	// TODO: Save state for HMR
	return render(Demo, element);
}
