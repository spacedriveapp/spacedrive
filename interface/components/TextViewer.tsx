import Prism from 'prismjs';
import { memo, useEffect, useState } from 'react';
import './prism.css';

export interface TEXTViewerProps {
	src: string;
	onLoad?: (event: HTMLElementEventMap['load']) => void;
	onError?: (event: HTMLElementEventMap['error']) => void;
	className?: string;
	syntaxHighlight?: boolean;
}

export const TEXTViewer = memo(
	({ src, onLoad, onError, className, syntaxHighlight }: TEXTViewerProps) => {
		// Ignore empty urls
		const href = !src || src === '#' ? null : src;
		const [quickPreviewContent, setQuickPreviewContent] = useState('');

		const loadContent = async () => {
			if (!href) return;
			const response = await fetch(href);
			if (!response.ok) return onError();
			response.text().then((text) => {
				onLoad();
				setQuickPreviewContent(text);
			});
		};
		loadContent();

		useEffect(() => {
			syntaxHighlight && Prism.highlightAll();
		}, [quickPreviewContent]);

		return (
			<pre
				className={className}
				style={{ wordWrap: 'break-word', whiteSpace: 'pre-wrap', colorScheme: 'dark' }}
			>
				{syntaxHighlight ? <code>{quickPreviewContent}</code> : quickPreviewContent}
			</pre>
		);
	}
);
