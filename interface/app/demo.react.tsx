import { useEffect, useState } from 'react';
import { WithSolid } from '@sd/client';

import { Demo3, demoCtx, SolidSquare } from './demo.solid';

export function Demo(props: { demo: string }) {
	const [count, setCount] = useState(0);
	const ctx = demoCtx.useContext();

	return (
		<div className="bg-green-500 p-2">
			<demoCtx.Provider value="set in react">
				<>
					<button onClick={() => setCount((count) => count + 1)} className="border p-1">
						Click me
					</button>
					<div>Hello from React: {count}</div>
					<div>{props.demo}</div>
					<div>CTX: {ctx()}</div>
					<Inner />
					<WithSolid root={Demo3} demo={count.toString()} />
				</>
			</demoCtx.Provider>
			<WithSolid root={Demo3} demo={count.toString()} />
			<SolidSquareManager />
		</div>
	);
}

function Inner() {
	const ctx = demoCtx.useContext();
	return null;
}

export function Demo2() {
	return null;
}

export function ReactSquare(props: { x: number; y: number }) {
	return (
		<div
			className="absolute z-[999999999] border bg-red-500"
			style={{ width: '30px', height: '30px', left: props.x + 'px', top: props.y + 'px' }}
		/>
	);
}

export function SolidSquareManager() {
	const [pos, setPos] = useState({ x: 75, y: 0, enabled: true });

	useEffect(() => {
		const interval = setInterval(
			() =>
				setPos((p) => {
					if (!p.enabled) return p;
					if (p.x > window.innerWidth) return { x: 100, y: 0, enabled: true };
					if (p.y > window.innerHeight) return { x: 0, y: 0, enabled: true };
					return { x: p.x + 1, y: p.y + 1, enabled: true };
				}),
			10
		);
		return () => clearInterval(interval);
	});

	return (
		<>
			<button onClick={() => setPos((p) => ({ ...p, enabled: !p.enabled }))}>
				Toggle Solid (blue)
			</button>
			<WithSolid root={SolidSquare} x={pos.x} y={pos.y} />
		</>
	);
}
