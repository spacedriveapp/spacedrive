/** @jsxImportSource solid-js */

import { createSignal } from 'solid-js';
import { createSharedContext, WithReact } from '@sd/client';

import { Demo as ReactDemo, Demo2 as ReactDemo2 } from './demo.react';

export const demoCtx = createSharedContext('Hello From Solid');

export function Demo(props: { demo: string }) {
	const [count, setCount] = createSignal(0);
	const [ctxValue, setCtxValue] = createSignal('set in solid');

	return (
		<demoCtx.Provider value={ctxValue()}>
			<div class="absolute top-0 z-[99999] bg-red-500">
				<button onClick={() => setCount((count) => count + 1)}>Click me</button>
				<button onClick={() => setCtxValue((s) => `${s}1`)}>Update ctx</button>
				<div>Hello from Solid: {count()}</div>
				<div>CTX: {props.demo}</div>
				<Inner />
				<WithReact root={ReactDemo} demo={count().toString()} />
				<WithReact root={ReactDemo2} />
			</div>
		</demoCtx.Provider>
	);
}

function Inner() {
	const ctx = demoCtx.useContext();
	console.log('FROM SOLID', ctx);
	return <div>CTX: {ctx}</div>;
}

export function Demo2() {
	return null;
}

export function Demo3(props: { demo: string }) {
	const ctx = demoCtx.useContext();
	return (
		<div class="bg-blue-500">
			<div>Hello from Solid again: {props.demo}</div>
			<div>CTX: {ctx}</div>
		</div>
	);
}
