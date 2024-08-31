import { IBM_Plex_Sans, Inter } from 'next/font/google';

export const plexSansFont = IBM_Plex_Sans({
	weight: ['300', '400', '600', '700'],
	subsets: ['latin'],
	display: 'swap',
	variable: '--font-plex-sans'
});

export const interFont = Inter({
	subsets: ['latin'],
	display: 'swap',
	variable: '--font-inter'
});
