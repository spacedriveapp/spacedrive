import clsx from 'clsx';
import { useEffect, useRef, useState } from 'react';
import { useIsDark } from '~/hooks';

import { useExplorerContext } from '../../Context';
import { useExplorerViewContext } from '../Context';

export const DATE_HEADER_HEIGHT = 140;

// million-ignore
export const DateHeader = ({ date }: { date?: string }) => {
	const isDark = useIsDark();

	const explorer = useExplorerContext();
	const view = useExplorerViewContext();

	const ref = useRef<HTMLDivElement>(null);

	const [isSticky, setIsSticky] = useState(false);

	useEffect(() => {
		const node = ref.current;
		if (!node) return;

		const scroll = explorer.scrollRef.current;
		if (!scroll) return;

		// We add the top of the explorer scroll because of the custom border/frame on desktop
		const rootMarginTop = (view.scrollPadding?.top ?? 0) + scroll.getBoundingClientRect().top;

		const observer = new IntersectionObserver(
			([entry]) => entry && setIsSticky(!entry.isIntersecting),
			{ rootMargin: `-${rootMarginTop}px 0px 0px 0px`, threshold: [1] }
		);

		observer.observe(node);
		return () => observer.disconnect();
	}, [explorer.scrollRef, view.scrollPadding?.top]);

	return (
		<div
			ref={ref}
			style={{ height: DATE_HEADER_HEIGHT }}
			className={clsx(
				'pointer-events-none sticky inset-x-0 -top-px z-10 p-5 transition-colors duration-500',
				!isSticky && !isDark ? 'text-ink' : 'text-white'
			)}
		>
			<div
				className={clsx(
					'absolute inset-0 bg-gradient-to-b from-black/60 to-transparent transition-opacity duration-500',
					isSticky ? 'opacity-100' : 'opacity-0'
				)}
			/>
			<div className={clsx('relative text-xl font-semibold', !date && 'opacity-75')}>
				{date ?? 'No date'}
			</div>
		</div>
	);
};
