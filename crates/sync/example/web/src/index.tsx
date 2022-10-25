/* @refresh reload */
import { Suspense } from 'react';
import { render } from 'react-dom';
import { createRoot } from 'react-dom/client';

import { App } from './App';
import './index.css';
import { queryClient, rspc, rspcClient } from './utils/rspc';

const root = createRoot(document.getElementById('root') as HTMLElement).render(
	<rspc.Provider client={rspcClient} queryClient={queryClient}>
		<Suspense fallback={null}>
			<App />
		</Suspense>
	</rspc.Provider>
);
