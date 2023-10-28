import { MetadataRoute } from 'next';

export default function manifest(): MetadataRoute.Manifest {
	return {
		name: 'Spacedrive',
		theme_color: '#E751ED',
		start_url: '/',
		icons: [
			{
				src: '/images/logo-192x192.png',
				sizes: '192x192',
				type: 'image/png'
			},
			{
				src: '/images/logo-512x512.png',
				sizes: '512x512',
				type: 'image/png'
			}
		]
	};
}
