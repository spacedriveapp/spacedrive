/** @jsxImportSource solid-js */

import { createSignal } from 'solid-js';
import { createSharedContext, WithReact } from '@sd/client';

import { Demo as ReactDemo, Demo2 as ReactDemo2 } from './demo.react';

export const demoCtx = createSharedContext('Hello From Solid');

export function Demo(props: { demo: string }) {
	const [count, setCount] = createSignal(0);

	return (
		<demoCtx.Provider value="todo">
			<div class="absolute top-0 z-[99999] bg-red-500">
				<button onClick={() => setCount((count) => count + 1)}>Click me</button>
				<div>Hello from Solid: {count()}</div>
				<div>{props.demo}</div>
				<WithReact root={ReactDemo} demo={count().toString()} />
				<WithReact root={ReactDemo2} />
				<Inner />
			</div>
		</demoCtx.Provider>
	);
}

function Inner() {
	const ctx = demoCtx.useContext();
	console.log('FROM SOLID', ctx);
	return null;
}

export function Demo2() {
	return null;
}

export function Demo3(props: { demo: string }) {
	return <div>{props.demo}</div>;
}
