import Prism from 'prismjs';
import { memo, useLayoutEffect, useMemo, useState } from 'react';
import './prism.css';

export interface CODEViewerProps {
	src: string;
	onLoad?: (event: HTMLElementEventMap['load']) => void;
	onError?: (event: HTMLElementEventMap['error']) => void;
	className?: string;
	crossOrigin?: React.ComponentProps<'link'>['crossOrigin'];
}

export const CODEViewer = memo(
	({ src, onLoad, onError, className, crossOrigin }: CODEViewerProps) => {
		// Ignore empty urls
		const href = !src || src === '#' ? null : src;
		const [quickPreviewContent, setQuickPreviewContent] = useState('');

		// Use link preload as a hack to get access to an onLoad and onError events for the object tag
		// as well as to normalize the URL
		const link = useMemo(() => {
			if (href == null) return null;

			const link = document.createElement('link');
			link.as = 'fetch';
			link.rel = 'preload';
			if (crossOrigin) link.crossOrigin = crossOrigin;
			link.href = href;

			link.addEventListener('load', () => link.remove());
			link.addEventListener('error', () => link.remove());

			return link;
		}, [crossOrigin, href]);

		// The useLayoutEffect is used to ensure that the event listeners are added before the object is loaded
		// The useLayoutEffect declaration order is important here
		useLayoutEffect(() => {
			if (!link) return;

			if (onLoad) link.addEventListener('load', onLoad);
			if (onError) link.addEventListener('error', onError);

			return () => {
				if (onLoad) link.removeEventListener('load', onLoad);
				if (onError) link.removeEventListener('error', onError);
			};
		}, [link, onLoad, onError]);

		useLayoutEffect(() => {
			if (!link) return;
			document.head.appendChild(link);

			const loadContent = async () => {
				if (link.href) {
					const response = await fetch(link.href);
					if (response.ok) {
						response.text().then((text) => {
							setQuickPreviewContent(text);
							Prism.highlightAll();
						});
					}
				}
			};
			loadContent();

			return () => link.remove();
		}, [link]);

		return link ? (
			<pre
				className={className}
				style={{ wordWrap: 'break-word', whiteSpace: 'pre-wrap', colorScheme: 'dark' }}
			>
				<code>{quickPreviewContent}</code>
			</pre>
		) : null;
	}
);
