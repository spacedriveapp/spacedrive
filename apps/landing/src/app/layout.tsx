import type { Metadata, Viewport } from 'next';
import { PropsWithChildren } from 'react';
import { GlobalFooter } from '~/components/global-footer';
import { NavBar } from '~/components/global-nav-bar';

import '@sd/ui/style/style.scss';
import '~/styles/prism.css';
import '~/styles/style.scss';

import clsx from 'clsx';
import PlausibleProvider from 'next-plausible';

// import { DisclaimerBanner } from '~/components/disclaimer-banner';

import { ClientProviders } from './client-providers';
import { interFont, plexSansFont } from './fonts';

export const metadata: Metadata = {
	metadataBase: new URL('https://spacedrive.com'),
	robots: 'index, follow',
	description:
		'Combine your drives and clouds into one database that you can organize and explore from any device. Designed for creators, hoarders and the painfully disorganized.',
	openGraph: {
		images: 'https://spacedrive.com/logo.png'
	},
	keywords:
		'files,file manager,spacedrive,file explorer,vdfs,distributed filesystem,cas,content addressable storage,virtual filesystem,photos app, video organizer,video encoder,tags,tag based filesystem',
	authors: { name: 'Spacedrive Technology Inc.', url: 'https://spacedrive.com' }
};

export const viewport: Viewport = {
	themeColor: [
		// background color in Safari
		{ color: '#0E0E12', media: 'screen' },
		// MUST BE LAST to actually work on embeds
		// embed color on discord, for instance
		{ color: '#E751ED', media: 'not screen' }
	]
};

export default function Layout({ children }: PropsWithChildren) {
	return (
		<html
			lang="en"
			className={clsx('scroll-smooth text-white', plexSansFont.variable, interFont.variable)}
		>
			<head>
				<PlausibleProvider
					domain="spacedrive.com"
					customDomain="spacedrive.com"
					trackOutboundLinks
					taggedEvents
				/>
			</head>
			<body className="noise noise-strongest min-h-screen bg-[#090909] font-plex">
				<div className="flex min-h-screen flex-col">
					{/* <DisclaimerBanner /> */}
					<ClientProviders>
						<NavBar />
						<main className="z-10 m-auto w-full max-w-[100rem] flex-1">{children}</main>
						<GlobalFooter />
					</ClientProviders>
				</div>
			</body>
		</html>
	);
}
