import { defineDocumentType, makeSource } from '@contentlayer/source-files';
import readingTime from 'reading-time';
import rehypeAutolinkHeadings from 'rehype-autolink-headings';
import rehypeSlug from 'rehype-slug';
import remarkGfm from 'remark-gfm';

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
		title: { type: 'string', required: true },
		index: { type: 'number', required: true, default: 100 }
	},
	computedFields: {
		url: { type: 'string', resolve: (post) => `/docs/${post._raw.flattenedPath}` },
		slug: {
			type: 'string',
			resolve: (p) => p._raw.flattenedPath.replace(/^.+?(\/)/, '')
		},
		categoryName: {
			type: 'string',
			resolve: (p) => p._raw.flattenedPath.replace(/^.+?(\/)/, '').split('/')[0]
		}
	}
}));

export default makeSource({
	contentDirPath: '../../', // project dir
	contentDirInclude: ['docs', 'apps/landing/posts'],
	documentTypes: [Post, Document],
	mdx: { remarkPlugins: [remarkGfm], rehypePlugins: [rehypeSlug, rehypeAutolinkHeadings] }
});
