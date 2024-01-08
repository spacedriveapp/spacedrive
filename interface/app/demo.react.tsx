import { useState } from 'react';
import { WithSolid } from '@sd/client';

import { Demo3 } from './demo.solid';

export function Demo(props: { demo: string }) {
	const [count, setCount] = useState(0);

	return (
		<div>
			<button onClick={() => setCount((count) => count + 1)}>Click me</button>
			<div>Hello from React: {count}</div>
			<div>{props.demo}</div>
			<WithSolid root={Demo3} demo={count.toString()} />
		</div>
	);
}

export function Demo2() {
	return null;
}
