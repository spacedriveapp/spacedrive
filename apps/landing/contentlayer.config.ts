import { defineDocumentType, makeSource } from 'contentlayer2/source-files';
import readingTime from 'reading-time';
// support for anchor links
import rehypeAutolinkHeadings from 'rehype-autolink-headings';
// adds rel to external links
import rehypeExternalLinks from 'rehype-external-links';
// support for math
import rehypeKatex from 'rehype-katex';
// support for code syntax highlighting
import rehypePrism from 'rehype-prism-plus';
// adds slug to headings
import rehypeSlug from 'rehype-slug';
// support for github flavored markdown
import remarkGfm from 'remark-gfm';
// support for math
import remarkMath from 'remark-math';
import remarkMdxImages from 'remark-mdx-images';

// adds width and height to images
import rehypeImageSize from './src/plugins/rehype-image-size';

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
		description: {
			type: 'string',
			description: 'Used for SEO and social media sharing',
			required: false,
			default: ''
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
		remarkPlugins: [
			remarkGfm,
			remarkMath,
			remarkMdxImages // does this even do anything??
		],
		rehypePlugins: [
			[rehypeImageSize, { root: `${process.cwd()}/public` }],
			rehypeSlug,
			rehypeAutolinkHeadings,
			rehypeKatex,
			rehypePrism,
			rehypeExternalLinks
		]
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
