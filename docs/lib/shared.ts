import {createGetUrl} from 'fumadocs-core/source';

export const appName = 'Spacedrive';
export const docsRoute = '/';
export const docsImageRoute = '/og/docs';
export const docsContentRoute = '/llms.mdx/docs';

export const gitConfig = {
	user: 'spacedriveapp',
	repo: 'spacedrive',
	branch: 'main'
};

const getContentUrl = createGetUrl(docsContentRoute);

export function getPageMarkdownUrl(page: {slugs: string[]; locale?: string}) {
	const segments = [...page.slugs, 'content.md'];

	return {segments, url: getContentUrl(segments, page.locale)};
}

const getImageUrl = createGetUrl(docsImageRoute);

export function getPageImageUrl(page: {slugs: string[]; locale?: string}) {
	const segments = [...page.slugs, 'image.png'];

	return {segments, url: getImageUrl(segments, page.locale)};
}
