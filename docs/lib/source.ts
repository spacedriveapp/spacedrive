import {llms, loader} from 'fumadocs-core/source';
import {lucideIconsPlugin} from 'fumadocs-core/source/lucide-icons';
import {metaSchema, pageSchema} from 'fumadocs-core/source/schema';
import {defineDocs} from 'fumadocs-mdx/macro';
import {z} from 'zod';
import {docsRoute} from './shared';

// Content lives at the repo's docs root, alongside internal plans and design
// notes that stay out of the site. Only the published sections are collected.
const docs = defineDocs({
	dir: '.',
	docs: {
		schema: pageSchema.extend({
			sidebarTitle: z.string().optional()
		}),
		files: ['{overview,core,extensions,cli,react,gpui}/**/*.mdx'],
		postprocess: {
			includeProcessedMarkdown: true
		}
	},
	meta: {
		schema: metaSchema,
		files: [
			'meta.json',
			'{overview,core,extensions,cli,react,gpui}/**/meta.json'
		]
	}
});

export const source = loader({
	baseUrl: docsRoute,
	source: docs.toFumadocsSource(),
	plugins: [lucideIconsPlugin()],
	pageTree: {
		transformers: [
			{
				// Pages can set `sidebarTitle` to show a shorter name in the sidebar
				// than the full page title.
				file(node, filePath) {
					if (!filePath) return node;
					const file = this.storage.read(filePath);
					if (file?.format === 'page') {
						const {sidebarTitle} = file.data as {
							sidebarTitle?: string;
						};
						if (sidebarTitle) node.name = sidebarTitle;
					}
					return node;
				}
			}
		]
	}
});

export const docsLlms = llms(source, {
	renderPage: async (page) => `# ${page.data.title} (${page.url})

${await page.data.getText('processed')}`
});
