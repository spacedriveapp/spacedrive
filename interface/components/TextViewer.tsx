import Prism from 'prismjs';
import { memo, useState } from 'react';
import './prism.css';

export interface TextViewerProps {
	src: string;
	onLoad?: (event: HTMLElementEventMap['load']) => void;
	onError?: (event: HTMLElementEventMap['error']) => void;
	className?: string;
	syntaxHighlight?: boolean;
}

export const TextViewer = memo(
	({ src, onLoad, onError, className, syntaxHighlight }: TextViewerProps) => {
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
				syntaxHighlight && Prism.highlightAll();
			});
		};
		loadContent();

		return (
			<pre className={className} style={{ colorScheme: 'dark' }}>
				{syntaxHighlight ? <code>{quickPreviewContent}</code> : quickPreviewContent}
			</pre>
		);
	}
);
