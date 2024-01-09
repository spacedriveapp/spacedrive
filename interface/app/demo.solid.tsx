/** @jsxImportSource solid-js */

import { createSignal } from 'solid-js';
import { createSharedContext, WithReact } from '@sd/client';

import { Demo as ReactDemo, Demo2 as ReactDemo2 } from './demo.react';

export const demoCtx = createSharedContext('the ctx was not set');

export function Demo(props: { demo: string }) {
	const [count, setCount] = createSignal(0);
	const [ctxValue, setCtxValue] = createSignal('set in solid');

	return (
		<div class="absolute top-0 z-[99999] bg-red-500 p-2">
			<demoCtx.Provider value={ctxValue()}>
				<button onClick={() => setCount((count) => count + 1)} class="border p-1">
					Click me
				</button>
				<button onClick={() => setCtxValue((s) => `${s}1`)} class="ml-4 border p-1">
					Update ctx
				</button>
				<div>Hello from Solid: {count()}</div>
				<div>CTX: {props.demo}</div>
				<Inner />
				<WithReact root={ReactDemo} demo={count().toString()} />
				<WithReact root={ReactDemo2} />
			</demoCtx.Provider>
		</div>
	);
}

function Inner() {
	const ctx = demoCtx.useContext();
	console.log('FROM SOLID', ctx());
	return <div>CTX: {ctx()}</div>;
}

export function Demo2() {
	return null;
}

export function Demo3(props: { demo: string }) {
	const ctx = demoCtx.useContext();

	return (
		<div class="m-2 bg-blue-500 p-2">
			<div>Hello from Solid again: {props.demo}</div>
			<div>CTX: {ctx()}</div>
		</div>
	);
}
