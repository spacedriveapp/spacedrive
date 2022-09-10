import parseMarkdownMetadata from 'markdown-yaml-metadata-parser';

export interface MarkdownPageData {
	name?: string;
	index?: number;
	new?: boolean;
}

interface MarkdownParsed {
	render: string;
	data?: MarkdownPageData;
}

export function parseMarkdown(markdownAsHtml: string, markdownRaw: string): MarkdownParsed {
	let metadata: MarkdownPageData | undefined = undefined;

	try {
		metadata = parseMarkdownMetadata(markdownRaw).metadata as MarkdownPageData;
	} catch (e) {
		// console.warn('failed to parse markdown', e);
		// this doesn't matter
	}

	// make all non local links open in new tab
	markdownAsHtml = markdownAsHtml.replaceAll(
		'<a href=',
		`<a target="_blank" rel="noreferrer" href=`
	);

	return {
		render: markdownAsHtml,
		data: metadata
	};
}
