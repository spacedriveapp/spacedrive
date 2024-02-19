import { BloomOne } from '@sd/assets/images';
import { introvideobg, sdintro } from '@sd/assets/videos';
import clsx from 'clsx';
import { useLayoutEffect, useState } from 'react';
import { Navigate, Outlet } from 'react-router';
import { useDebugState } from '@sd/client';
import DragRegion from '~/components/DragRegion';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';

import DebugPopover from '../$libraryId/Layout/Sidebar/DebugPopover';
import { macOnly } from '../$libraryId/Layout/Sidebar/helpers';
import { OnboardingContext, useContextValue } from './context';
import Progress from './Progress';

export const Component = () => {
	const os = useOperatingSystem(false);
	const debugState = useDebugState();
	// FIX-ME: Intro video breaks onboarding for the web and Linux versions
	const [showIntro, setShowIntro] = useState(os === 'macOS' || os === 'windows');
	const [windowSize, setWindowSize] = useState({
		width: window.innerWidth,
		height: window.innerHeight
	});
	const ctx = useContextValue();

	useLayoutEffect(() => {
		const handleResize = () => {
			setWindowSize({ width: window.innerWidth, height: window.innerHeight });
		};
		window.addEventListener('resize', handleResize);
		return () => window.removeEventListener('resize', handleResize);
	}, []);

	if (ctx.libraries.isLoading) return null;
	if (ctx.library?.uuid !== undefined) return <Navigate to={`/${ctx.library.uuid}`} replace />;

	return (
		<OnboardingContext.Provider value={ctx}>
			<div
				className={clsx(
					macOnly(os, 'bg-opacity-[0.75]'),
					'flex h-screen flex-col bg-sidebar text-ink'
				)}
			>
				{showIntro && (
					<div className="absolute top-0 left-0 z-50 flex items-center justify-center w-screen h-screen">
						<svg
							width="100%"
							height="100%"
							className="absolute left-0 top-0 z-[-1]"
							viewBox={`0 0 ${windowSize.width} ${windowSize.height}`}
							fill="none"
							xmlns="http://www.w3.org/2000/svg"
						>
							<rect width="100%" height="100%" fill="#2E2F38" />
						</svg>
						<video
							style={{
								position: 'absolute',
								objectFit: 'cover',
								width: '100vw',
								height: '100vh',
								zIndex: -1
							}}
							preload="auto"
							src={introvideobg}
							muted
							controls={false}
						/>
						<video
							className="mx-auto w-[700px]"
							autoPlay
							onEnded={() => {
								setShowIntro(false);
							}}
							muted
							controls={false}
							src={sdintro}
						/>
					</div>
				)}
				<DragRegion className="z-50 h-9" />
				<div className="flex flex-col gap-8 p-10 -mt-5 grow">
					<div className="flex flex-col items-center justify-center grow">
						<Outlet />
					</div>
					<Progress />
				</div>
				<div className="flex justify-center p-4">
					<p className="text-xs opacity-50 text-ink-dull">
						&copy; {new Date().getFullYear()} Spacedrive Technology Inc.
					</p>
				</div>
				<div className="absolute -z-10">
					<div className="relative w-screen h-screen">
						<img src={BloomOne} className="absolute h-[2000px] w-[2000px]" />
						{/* <img src={BloomThree} className="absolute w-[2000px] h-[2000px] -right-[200px]" /> */}
					</div>
				</div>
				{debugState.enabled && <DebugPopover />}
			</div>
		</OnboardingContext.Provider>
	);
};
