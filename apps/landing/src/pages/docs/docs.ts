import {
	BuildingLibraryIcon,
	CodeBracketIcon,
	CubeIcon,
	FolderIcon,
	InformationCircleIcon,
	SparklesIcon,
	StarIcon
} from '@heroicons/react/24/solid';

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
			icon: SparklesIcon
		},
		{
			title: 'Developers',
			slug: 'developers',
			icon: CubeIcon
		},
		{
			title: 'Company',
			slug: 'company',
			icon: BuildingLibraryIcon
		},
		{
			title: 'Changelog',
			slug: 'changelog',
			icon: StarIcon
		}
	]
};

export default config;
