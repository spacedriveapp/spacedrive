import { useLayoutEffect } from 'react';

export interface ExternalObjectProps
	extends Omit<React.ComponentProps<'object'>, 'chidren' | 'onLoad' | 'onError'> {
	data: string;
	onLoad?: (event: Event) => void;
	onError?: (event: string | Event) => void;
	crossOrigin?: React.ComponentProps<'link'>['crossOrigin'];
}

export const ExternalObject = ({
	data,
	onLoad,
	onError,
	crossOrigin,
	...props
}: ExternalObjectProps) => {
	// Use link preload as a hack to get access to an onLoad and onError events for the object tag
	useLayoutEffect(
		() => {
			if (!(onLoad && onError)) return;

			const link = document.createElement('link');
			link.as = 'fetch';
			link.rel = 'preload';
			if (crossOrigin) link.crossOrigin = crossOrigin;
			link.href = data;

			link.onload = (e) => {
				link.remove();
				onLoad(e);
			};

			link.onerror = (e) => {
				link.remove();
				onError(e);
			};

			document.head.appendChild(link);

			() => link.remove();
		},
		// Disable this rule because onLoad and onError changed should not trigger this effect
		// TODO: Remove this after useEffectEvent enters stable React
		// https://react.dev/learn/separating-events-from-effects#declaring-an-effect-event
		// eslint-disable-next-line react-hooks/exhaustive-deps
		[data, crossOrigin]
	);

	return <object data={data} {...props} />;
};
