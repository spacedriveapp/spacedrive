import { defineDocumentType, makeSource } from '@contentlayer/source-files';
import readingTime from 'reading-time';
import rehypeAutolinkHeadings from 'rehype-autolink-headings';
import rehypeKatex from 'rehype-katex';
import rehypeSlug from 'rehype-slug';
import remarkGfm from 'remark-gfm';
import remarkMath from 'remark-math';

// Blog
export const Post = defineDocumentType(() => ({
	name: 'Post',
	filePathPattern: `apps/landing/posts/**/*.mdx`,
	contentType: 'mdx',
	fields: {
		title: { type: 'string', required: true },
		author: { type: 'string', default: 'Spacedrive Technology Inc.' },
		date: { type: 'date', required: true },
		image: {
			type: 'string',
			description: 'Hero Image URL',
			// TODO: Change this to a generic default image
			default: 'https://avatars.githubusercontent.com/u/101227423?s=200&v=4'
		},
		imageAlt: { type: 'string', description: 'Hero Image Alt' },
		excerpt: { type: 'string' },
		tags: {
			type: 'list',
			of: { type: 'string' },
			required: true
		}
	},
	computedFields: {
		url: {
			type: 'string',
			resolve: (post) => `/blog/${post._raw.sourceFileName.replace(/\.mdx$/, '')}`
		},
		slug: {
			type: 'string',
			resolve: (post) => post._raw.sourceFileName.replace(/\.mdx$/, '')
		},
		readTime: { type: 'string', resolve: (doc) => readingTime(doc.body.raw).text },
		image: {
			type: 'string',
			resolve: (post) => (post.image.startsWith('http') ? post.image : `/${post.image}`)
		}
	}
}));

// Docs
export const Document = defineDocumentType(() => ({
	name: 'Doc',
	filePathPattern: `docs/**/*.mdx`,
	contentType: 'mdx',
	fields: {
		title: {
			type: 'string',
			description: 'Title of the document, if nothing is provided file name will be used'
		},
		index: {
			type: 'number',
			default: 100,
			description:
				'Order of the document, if nothing is provided, 100 is default. This is relative to the other docs in the category. Group of lower indexes (categories) will be shown first.'
		}
	},
	computedFields: {
		url: { type: 'string', resolve: (post) => `/${post._raw.flattenedPath}` },
		slug: {
			type: 'string',
			resolve: (p) => p._raw.flattenedPath.replace(/^.+?(\/)/, '')
		},
		title: {
			type: 'string',
			resolve: (p) =>
				p.title
					? toTitleCase(p.title)
					: toTitleCase(
							p._raw.flattenedPath
								.replace(/^.+?(\/)/, '')
								.split('/')
								.slice(-1)[0]
					  )
		},
		section: {
			type: 'string',
			resolve: (p) => p._raw.flattenedPath.replace(/^.+?(\/)/, '').split('/')[0]
		},
		category: {
			type: 'string',
			resolve: (p) => p._raw.flattenedPath.replace(/^.+?(\/)/, '').split('/')[1] || ''
		}
	}
}));

export default makeSource({
	contentDirPath: '../../', // project dir
	contentDirInclude: ['docs', 'apps/landing/posts'],
	documentTypes: [Post, Document],
	mdx: {
		remarkPlugins: [remarkGfm, remarkMath],
		rehypePlugins: [rehypeSlug, rehypeAutolinkHeadings, rehypeKatex]
	}
});

// Can't import the one in utils/util.ts so we have to duplicate it here
function toTitleCase(str: string) {
	return str
		.toLowerCase()
		.replace(/(?:^|[\s-/])\w/g, function (match) {
			return match.toUpperCase();
		})
		.replaceAll('-', ' ');
}
