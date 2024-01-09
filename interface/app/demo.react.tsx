import { useState } from 'react';
import { WithSolid } from '@sd/client';

import { Demo3, demoCtx } from './demo.solid';

export function Demo(props: { demo: string }) {
	const [count, setCount] = useState(0);

	const ctx = demoCtx.useContext();
	console.log('FROM REACT 1', ctx());

	return (
		<div className="bg-green-500 p-2">
			<demoCtx.Provider value="set in react">
				<>
					<button onClick={() => setCount((count) => count + 1)}>Click me</button>
					<div>Hello from React: {count}</div>
					<div>{props.demo}</div>
					<div>CTX: {ctx()}</div>
					<Inner />
					<WithSolid root={Demo3} demo={count.toString()} />
				</>
			</demoCtx.Provider>
			<WithSolid root={Demo3} demo={count.toString()} />
		</div>
	);
}

function Inner() {
	const ctx = demoCtx.useContext();
	console.log('FROM REACT 2', ctx());
	return null;
}

export function Demo2() {
	return null;
}
