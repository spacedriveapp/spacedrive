// WARNING: BE CAREFUL SAVING THIS FILE WITH A FORMATTER ENABLED. The import order is important and goes against prettier's recommendations.
import React, { Suspense } from 'react';
import ReactDOM from 'react-dom/client';
// THIS MUST GO BEFORE importing the App
import '~/patches';

import App from './App';

import '@sd/ui/style';

const root = ReactDOM.createRoot(document.getElementById('root') as HTMLElement);
root.render(
	<React.StrictMode>
		<Suspense>
			<App />
		</Suspense>
	</React.StrictMode>
);
