import { Head, Html, Main, NextScript } from 'next/document';

export default function Document() {
	return (
		<Html lang="en" className="dark">
			<Head>
				<meta charSet="UTF-8" />
				<link rel="icon" type="image/svg+xml" href="/favicon.ico" />
				<meta name="viewport" content="width=device-width, initial-scale=1.0" />
				<meta name="theme-color" content="#E751ED" media="not screen" />
				<meta name="robots" content="index, follow" />
			</Head>
			<body>
				<Main />
				<NextScript />
			</body>
		</Html>
	);
}
