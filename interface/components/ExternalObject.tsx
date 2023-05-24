import { memo, useLayoutEffect, useMemo } from 'react';

export interface ExternalObjectProps
	extends Omit<React.ComponentProps<'object'>, 'chidren' | 'onLoad' | 'onError'> {
	data: string;
	onLoad?: (event: HTMLElementEventMap['load']) => void;
	onError?: (event: HTMLElementEventMap['error']) => void;
	crossOrigin?: React.ComponentProps<'link'>['crossOrigin'];
}

export const ExternalObject = memo(
	({ data, onLoad, onError, crossOrigin, ...props }: ExternalObjectProps) => {
		// Ignore empty src
		const href = !data || data === '#' ? null : data;

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

		// Use link preload as a hack to get access to an onLoad and onError events for the object tag
		useLayoutEffect(() => {
			if (!link) return;

			if (onLoad) link.addEventListener('load', onLoad);
			if (onError) link.addEventListener('error', onError);

			document.head.appendChild(link);
			return () => {
				if (onLoad) link.removeEventListener('load', onLoad);
				if (onError) link.removeEventListener('error', onError);
				link.remove();
			};
		}, [link, onLoad, onError]);

		// Use link to normalize URL
		return link ? <object data={link.href} {...props} /> : null;
	}
);
