/* eslint-disable solid/reactivity */
/** @jsxImportSource solid-js */

import { createSignal } from 'solid-js';
import { createSharedContext, WithReact } from '@sd/client';

import { Demo as ReactDemo, Demo2 as ReactDemo2, ReactSquare } from './demo.react';

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
				<WithReact root={ReactDemo} demo={count().toString()} inner={true} />
				<WithReact root={ReactDemo2} />
			</demoCtx.Provider>
			<ReactSquareManager />
		</div>
	);
}

function Inner() {
	const ctx = demoCtx.useContext();
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

export function SolidSquare(props: { x: number; y: number }) {
	return (
		<div
			class="absolute z-[999999999] border bg-blue-500"
			style={{ width: '30px', height: '30px', left: props.x + 'px', top: props.y + 'px' }}
		/>
	);
}

export function ReactSquareManager() {
	const [pos, setPos] = createSignal({ x: 100, y: 0, enabled: true });

	setInterval(() => {
		setPos((p) => {
			if (!p.enabled) return p;
			if (p.x > window.innerWidth) return { x: 100, y: 0, enabled: true };
			if (p.y > window.innerHeight) return { x: 0, y: 0, enabled: true };
			return { x: p.x + 1, y: p.y + 1, enabled: true };
		});
	}, 10);

	return (
		<>
			<button onClick={() => setPos((p) => ({ ...p, enabled: !p.enabled }))}>
				Toggle React (red)
			</button>
			<WithReact root={ReactSquare} x={pos().x} y={pos().y} />
		</>
	);
}
