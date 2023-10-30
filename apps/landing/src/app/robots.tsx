import { MetadataRoute } from 'next';

export default function robots(): MetadataRoute.Robots {
	return {
		rules: {
			userAgent: '*',
			allow: '/',
			disallow: [
				'/api/',
				'/images/app/',
				'/images/bloom/',
				'/images/misc/',
				'/images/cloud-providers/'
			]
		},
		sitemap: 'https://spacedrive.com/sitemap.xml'
	};
}
