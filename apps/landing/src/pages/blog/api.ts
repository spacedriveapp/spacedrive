import { parseMarkdown } from '../../utils/markdownParse';

// import { teamImages, teamMembers } from '../team.page';

const posts = import.meta.glob('./posts/*.md', { as: 'raw', eager: true });

export interface BlogPost {
	title: string;
	slug: string;
	author: string;
	date: string;
	image?: string;
	excerpt?: string;
	imageCaption?: string;
	tags: string[];
	readTime?: string;
	html?: string;
}

export type BlogPosts = Record<string, BlogPost>;

export function getBlogPosts(): BlogPosts {
	const parsedPosts: Record<string, BlogPost> = {};
	Object.keys(posts).forEach((path) => {
		const slug = parsePathToName(path);
		if (!slug) return null;

		const { render, metadata } = parseMarkdown(posts[path]!);

		// const author = teamMembers.find((member) => member.name === metadata?.author || '');

		parsedPosts[slug] = {
			slug,
			title: metadata?.title || '',
			date: metadata?.date || '',
			author: metadata?.author || '',
			// author: {
			// 	name: author?.name || 'Spacedrive Team',
			// 	avatar:
			// 		teamImages[`${metadata?.author || ''}.jpg`] ||
			// 		'https://avatars.githubusercontent.com/u/101227423?v=4'
			// },
			image: metadata?.image || '',
			imageCaption: metadata?.imageCaption || '',
			tags: metadata?.tags?.split(',').map((tag) => tag.trim()) || [],
			readTime: metadata?.readTime,
			html: render
		};
	});

	return parsedPosts;
}

export function getBlogPost(name: string): BlogPost {
	const posts = getBlogPosts();
	return posts[name]!;
}

function parsePathToName(path: string): string | null {
	const name = path.split('posts/')[1]!.split('.md')[0];
	return name || null;
}

export function toTitleCase(str: string) {
	return str
		.toLowerCase()
		.replace(/(?:^|[\s-/])\w/g, function (match) {
			return match.toUpperCase();
		})
		.replaceAll('-', ' ');
}
