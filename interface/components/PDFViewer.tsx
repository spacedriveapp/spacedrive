import { memo, useLayoutEffect, useMemo } from 'react';

import { useOperatingSystem } from '~/hooks';

export interface PDFViewerProps {
	src: string;
	onLoad?: (event: HTMLElementEventMap['load']) => void;
	onError?: (event: HTMLElementEventMap['error']) => void;
	className?: string;
	crossOrigin?: React.ComponentProps<'link'>['crossOrigin'];
}

export const PDFViewer = memo(
	({ src, onLoad, onError, className, crossOrigin }: PDFViewerProps) => {
		const os = useOperatingSystem(true);
		// Ignore empty urls
		const href = !src || src === '#' ? null : src;

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
			return () => link.remove();
		}, [link]);

		// Use link to normalize URL
		return link ? (
			os === 'macOS' ? (
				// FIX-ME: Using <embed> isn't working in macOS for some reason
				<iframe src={link.href} style={{ objectFit: 'unset' }} className={className} />
			) : (
				<embed src={link.href} type="application/pdf" className={className} />
			)
		) : null;
	}
);
