import { snapshot } from 'valtio';
import { useNormalisedCache } from '@sd/client';

export function Component() {
	const cache = useNormalisedCache();
	const data = snapshot(cache['#cache']);
	return (
		<div className="p-4">
			<h1>Cache Debug</h1>
			<pre className="pt-4">{JSON.stringify(data, null, 2)}</pre>
		</div>
	);
}
