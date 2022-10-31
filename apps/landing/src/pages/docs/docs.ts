import { Circle, Cube, Sparkle, Star } from 'phosphor-react';

import { DocsConfig } from './api';

export function loadDocs() {
	return import.meta.glob('../../../../../docs/**/**/*.md', { as: 'raw', eager: true });
}

// in the end this will be passed into the inevitable vite plugin
const config: DocsConfig = {
	docs: loadDocs(),
	// for some stupid reason globEager as raw gives an incorrect type
	sections: [
		{
			title: 'Product',
			slug: 'product',
			icon: Sparkle
		},
		{
			title: 'Developers',
			slug: 'developers',
			icon: Cube
		},
		{
			title: 'Company',
			slug: 'company',
			icon: Circle
		},
		{
			title: 'Changelog',
			slug: 'changelog',
			icon: Star
		}
	]
};

export default config;
