// WARNING: BE CAREFUL SAVING THIS FILE WITH A FORMATTER ENABLED. The import order is important and goes against prettier's recommendations.
import React, { Suspense } from 'react';
import ReactDOM from 'react-dom/client';
// THIS MUST GO BEFORE importing the App
import '~/patches';
import App from './App';

// React dev tools extension
if (import.meta.env.DEV) {
	var script = document.createElement('script');
	script.src = 'http://localhost:8097';
	document.head.appendChild(script);
}

const root = ReactDOM.createRoot(document.getElementById('root') as HTMLElement);
root.render(
	<React.StrictMode>
		<Suspense>
			<App />
		</Suspense>
	</React.StrictMode>
);
