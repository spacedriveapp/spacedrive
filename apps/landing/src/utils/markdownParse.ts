export interface MarkdownPageData {
	name?: string;
	index?: number;
	new?: boolean;
}

interface MarkdownParsed {
	render: string;
	data: MarkdownPageData;
}

export function parseMarkdown(markdownAsHtml: string, markdownRaw: string): MarkdownParsed {
	if (markdownRaw.includes('# Objects')) console.log({ markdownAsHtml, markdownRaw });

	// make all non local links open in new tab
	markdownAsHtml = markdownAsHtml.replaceAll(
		'<a href=',
		`<a target="_blank" rel="noreferrer" href=`
	);

	return {
		render: markdownAsHtml,
		data: {
			name: 'jeff',
			index: 0,
			new: false
		}
	};
}
