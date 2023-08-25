import clsx from 'clsx';
import Prism from 'prismjs';
import { memo, useEffect, useRef, useState } from 'react';
import './prism.css';

// if you are intending to use Prism functions manually, you will need to set:
Prism.manual = true;

// Async import prism extras languages and plugins
import('./prism').catch(console.error);

export interface TextViewerProps {
	src: string;
	onLoad?: (event: HTMLElementEventMap['load']) => void;
	onError?: (event: HTMLElementEventMap['error']) => void;
	className?: string;
	syntaxHighlight?: string;
}

export const TextViewer = memo(
	({ src, onLoad, onError, className, syntaxHighlight }: TextViewerProps) => {
		// Ignore empty urls
		const ref = useRef<HTMLPreElement>(null);
		const href = !src || src === '#' ? null : src;
		const [quickPreviewContent, setQuickPreviewContent] = useState('');

		useEffect(() => {
			if (!href) return;

			const controller = new AbortController();

			fetch(href, { mode: 'cors', signal: controller.signal })
				.then((response) => {
					if (!response.ok) throw new Error(`Invalid response: ${response.statusText}`);
					return response.text();
				})
				.then((text) => {
					onLoad?.(new UIEvent('load', {}));
					setQuickPreviewContent(text);
				})
				.catch((error) => {
					if (!controller.signal.aborted)
						onError?.(new ErrorEvent('error', { message: `${error}` }));
				});

			return () => controller.abort();
		}, [href, onError, onLoad, syntaxHighlight]);

		useEffect(() => {
			const elem = ref.current;
			if (elem && syntaxHighlight && quickPreviewContent) Prism.highlightElement(elem);
		}, [syntaxHighlight, quickPreviewContent]);

		return (
			<pre
				className={clsx(
					className,
					syntaxHighlight && ['line-numbers', `language-${syntaxHighlight}`]
				)}
			>
				{syntaxHighlight ? (
					<code ref={ref}>{quickPreviewContent}</code>
				) : (
					quickPreviewContent
				)}
			</pre>
		);
	}
);
