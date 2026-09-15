import {RootProvider} from 'fumadocs-ui/provider/next';
import './global.css';
import type {Metadata} from 'next';

export const metadata: Metadata = {
	metadataBase: new URL('https://docs.spacedrive.com'),
	title: {
		template: '%s | Spacedrive Docs',
		default: 'Spacedrive Docs'
	},
	icons: {icon: '/favicon.png'}
};

export default function Layout({children}: LayoutProps<'/'>) {
	return (
		<html lang="en" className="font-sans" suppressHydrationWarning>
			<body className="flex min-h-screen flex-col">
				<RootProvider>{children}</RootProvider>
			</body>
		</html>
	);
}
