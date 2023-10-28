import { MetadataRoute } from 'next';

export default function sitemap(): MetadataRoute.Sitemap {
	return [
		{
			url: 'https://spacedrive.com',
			priority: 1
		},
		{
			url: 'https://spacedrive.com/docs',
			changeFrequency: 'always',
			priority: 0.9
		},
		{
			url: 'https://spacedrive.com/blog',
			priority: 0.8
		},
		// enable once this goes live
		// {
		// 	url: 'https://spacedrive.com/pricing',
		// 	priority: 0.8
		// },
		{
			url: 'https://spacedrive.com/roadmap',
			priority: 0.75
		},
		{
			url: 'https://spacedrive.com/team',
			priority: 0.65
		},
		{
			url: 'https://spacedrive.com/careers',
			priority: 0.65
		}
	];
}
