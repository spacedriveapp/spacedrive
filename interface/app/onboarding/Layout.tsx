import { BloomOne } from '@sd/assets/images';
import clsx from 'clsx';
import { useEffect } from 'react';
import { Outlet, useNavigate } from 'react-router';
import { getOnboardingStore, useDebugState } from '@sd/client';
import { tw } from '@sd/ui';
import DragRegion from '~/components/DragRegion';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';
import DebugPopover from '../$libraryId/Layout/Sidebar/DebugPopover';
import Progress from './Progress';

export const OnboardingContainer = tw.div`flex flex-col items-center`;
export const OnboardingTitle = tw.h2`mb-2 text-3xl font-bold`;
export const OnboardingDescription = tw.p`max-w-xl text-center text-ink-dull`;
export const OnboardingImg = tw.img`w-20 h-20 mb-2`;

export const Component = () => {
	const os = useOperatingSystem();
	const debugState = useDebugState();
	const navigate = useNavigate();

	useEffect(
		() => {
			const obStore = getOnboardingStore();

			// This is neat because restores the last active screen, but only if it is not the starting screen
			// Ignoring if people navigate back to the start if progress has been made
			if (obStore.unlockedScreens.length > 1) {
				navigate(`/onboarding/${obStore.lastActiveScreen}`);
			}
		},
		// eslint-disable-next-line react-hooks/exhaustive-deps
		[]
	);

	return (
		<div
			className={clsx(
				macOnly(os, 'bg-opacity-[0.75]'),
				'flex h-screen flex-col bg-sidebar text-ink'
			)}
		>
			<DragRegion className="z-50 h-9" />
			<div className="-mt-5 flex grow flex-col p-10">
				<div className="flex grow flex-col items-center justify-center">
					<Outlet />
				</div>
				<Progress />
			</div>
			<div className="flex justify-center p-4">
				<p className="text-xs text-ink-dull opacity-50">
					&copy; 2022 Spacedrive Technology Inc.
				</p>
			</div>
			<div className="absolute -z-10">
				<div className="relative h-screen w-screen">
					<img src={BloomOne} className="absolute h-[2000px] w-[2000px]" />
					{/* <img src={BloomThree} className="absolute w-[2000px] h-[2000px] -right-[200px]" /> */}
				</div>
			</div>
			{debugState.enabled && <DebugPopover />}
		</div>
	);
};

const macOnly = (platform: string | undefined, classnames: string) =>
	platform === 'macOS' ? classnames : '';
