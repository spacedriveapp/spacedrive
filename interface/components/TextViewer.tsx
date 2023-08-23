import Prism from 'prismjs';
import { memo, useEffect, useRef, useState } from 'react';
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

		useEffect(() => {
			if (!href) return;

			const controller = new AbortController();

			fetch(href, { signal: controller.signal })
				.then((response) => {
					if (!response.ok) throw new Error(`Invalid response: ${response.statusText}`);
					return response.text();
				})
				.then((text) => {
					onLoad?.(new UIEvent('load', {}));
					setQuickPreviewContent(text);
					if (syntaxHighlight) Prism.highlightAll();
				})
				.catch((error) => onError?.(new ErrorEvent('error', { message: `${error}` })));

			return () => controller.abort();
		}, [href, onError, onLoad, syntaxHighlight, quickPreviewContent]);

		return (
			<pre className={className}>
				{syntaxHighlight ? <code>{quickPreviewContent}</code> : quickPreviewContent}
			</pre>
		);
	}
);
