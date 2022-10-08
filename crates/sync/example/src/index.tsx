/* @refresh reload */
import { Suspense, render } from 'solid-js/web';

import { App } from './App';
import './index.css';
import { queryClient, rspc, rspcClient } from './rspc';

render(
	() => (
		<rspc.Provider client={rspcClient} queryClient={queryClient}>
			<Suspense fallback={null}>
				<App />
			</Suspense>
		</rspc.Provider>
	),
	document.getElementById('root') as HTMLElement
);
