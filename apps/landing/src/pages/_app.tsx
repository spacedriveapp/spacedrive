import type { AppProps } from 'next/app';
import Script from 'next/script';
import '@sd/ui/style';
import '~/styles/prism.css';
import '~/styles/style.scss';

export default function App({ Component, pageProps }: AppProps) {
	return (
		<>
			<Component {...pageProps} />
			<Script
				src="/stats/js/script.js"
				data-api="/stats/api/event"
				data-domain="spacedrive.com"
			/>
		</>
	);
}
