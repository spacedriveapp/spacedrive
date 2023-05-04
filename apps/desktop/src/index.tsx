// WARNING: Import order is important in this file. Make sure ~/patches comes before App.
import { StrictMode, Suspense } from 'react';
import ReactDOM from 'react-dom/client';
import '~/patches';
import App from './App';

const root = ReactDOM.createRoot(document.getElementById('root') as HTMLElement);
root.render(
	<StrictMode>
		<Suspense>
			<App />
		</Suspense>
	</StrictMode>
);
