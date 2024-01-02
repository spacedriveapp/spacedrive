/** @jsxImportSource solid-js */

import { createSignal } from 'solid-js';
import { render } from 'solid-js/web';

function Demo() {
	const [count, setCount] = createSignal(0);

	return (
		<div class="absolute top-0 bg-red-500">
			<button onClick={() => setCount(count() + 1)}>Click me</button>
			<div>Hello from Solid: {count()}</div>
		</div>
	);
}

// TODO: Get eslint error working for destructuring
function TestEslint({ demo }: { demo: string }) {}

export function renderDemo(element: HTMLDivElement) {
	console.log('Hello', Demo);

	render(() => <Demo />, document.getElementById('root2')!); // TODO: Remove this
}
