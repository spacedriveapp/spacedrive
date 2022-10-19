/* @refresh reload */
import { Suspense } from 'react';
import { render } from 'react-dom';
import { createRoot } from 'react-dom/client';

import { App } from './App';
import './index.css';
import { client, queryClient, rspc } from './utils/rspc';

const root = createRoot(document.getElementById('root') as HTMLElement).render(
	<rspc.Provider client={client} queryClient={queryClient}>
		<Suspense fallback={null}>
			<App />
		</Suspense>
	</rspc.Provider>
);
