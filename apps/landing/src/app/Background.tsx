'use client';

import dynamic from 'next/dynamic';
import { Suspense, useEffect, useState } from 'react';
import { ErrorBoundary } from 'react-error-boundary';
import { getWindow, hasWebGLContext } from '~/utils/util';

const FADE = {
	start: 300, // start fading out at 100px
	end: 1300 // end fading out at 300px
};

const Space = dynamic(() => import('~/components/Space'), { ssr: false });
const Bubbles = dynamic(() => import('~/components/Bubbles').then((m) => m.Bubbles), {
	ssr: false
});

export function Background() {
	const [opacity, setOpacity] = useState(0.6);
	const [isWindowResizing, setIsWindowResizing] = useState(false);

	useEffect(() => {
		const handleScroll = () => {
			const currentScrollY = window.scrollY;

			if (currentScrollY <= FADE.start) {
				setOpacity(0.6);
			} else if (currentScrollY <= FADE.end) {
				const range = FADE.end - FADE.start;
				const diff = currentScrollY - FADE.start;
				const ratio = diff / range;
				setOpacity(0.6 - ratio);
			} else {
				setOpacity(0);
			}
		};
		window.addEventListener('scroll', handleScroll);

		return () => {
			window.removeEventListener('scroll', handleScroll);
		};
	}, []);

	useEffect(() => {
		let resizeTimer: ReturnType<typeof setTimeout>;
		const handleResize = () => {
			setIsWindowResizing(true);
			clearTimeout(resizeTimer);
			resizeTimer = setTimeout(() => {
				setIsWindowResizing(false);
			}, 100);
		};
		window.addEventListener('resize', handleResize);
		return () => {
			window.removeEventListener('resize', handleResize);
			clearTimeout(resizeTimer);
		};
	}, []);

	return (
		<div style={{ opacity }}>
			<Suspense>
				{!isWindowResizing && (
					<ErrorBoundary
						fallbackRender={() => {
							console.warn(
								'Fallback to Bubbles background due WebGL not being available'
							);
							return <Bubbles />;
						}}
					>
						<Space />
					</ErrorBoundary>
				)}
			</Suspense>
		</div>
	);
}
