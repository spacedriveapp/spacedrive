/* @refresh reload */
import { Suspense, render } from 'solid-js/web';

import { App } from './App';
import './index.css';
import { client, queryClient, rspc } from './utils/rspc';

render(
	() => (
		<rspc.Provider client={client} queryClient={queryClient}>
			<Suspense fallback={null}>
				<App />
			</Suspense>
		</rspc.Provider>
	),
	document.getElementById('root') as HTMLElement
);
