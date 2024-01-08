import { useState } from 'react';
import { createSharedContext, WithSolid } from '@sd/client';

import { Demo3 } from './demo.solid';

export const demoCtx = createSharedContext('Hello From React');

export function Demo(props: { demo: string }) {
	const [count, setCount] = useState(0);

	return (
		<demoCtx.Provider value="todo">
			<div>
				<button onClick={() => setCount((count) => count + 1)}>Click me</button>
				<div>Hello from React: {count}</div>
				<div>{props.demo}</div>
				<WithSolid root={Demo3} demo={count.toString()} />
				<Inner />
			</div>
		</demoCtx.Provider>
	);
}

function Inner() {
	const ctx = demoCtx.useContext();
	console.log('FROM REACT', ctx);
	return null;
}

export function Demo2() {
	return null;
}
