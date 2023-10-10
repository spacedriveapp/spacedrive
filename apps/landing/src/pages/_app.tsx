import type { AppProps } from 'next/app';
import Head from 'next/head';
import Script from 'next/script';

import '@sd/ui/style';
import '~/styles/prism.css';
import '~/styles/style.scss';

export default function App({ Component, pageProps }: AppProps) {
	return (
		<>
			<Head>
				<meta name="viewport" content="width=device-width, initial-scale=1.0" />
			</Head>
			<Component {...pageProps} />
			<Script
				src="/stats/js/script.js"
				data-api="/stats/api/event"
				data-domain="spacedrive.com"
			/>
		</>
	);
}
