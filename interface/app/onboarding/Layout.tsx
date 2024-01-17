import { BloomOne } from '@sd/assets/images';
import clsx from 'clsx';
import { Navigate, Outlet } from 'react-router';
import { useDebugState } from '@sd/client';
import DragRegion from '~/components/DragRegion';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';

import DebugPopover from '../$libraryId/Layout/Sidebar/DebugPopover';
import { macOnly } from '../$libraryId/Layout/Sidebar/helpers';
import { OnboardingContext, useContextValue } from './context';
import Progress from './Progress';

export const Component = () => {
	const os = useOperatingSystem();
	const debugState = useDebugState();

	const ctx = useContextValue();

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
				<DragRegion className="z-50 h-9" />
				<div className="-mt-5 flex grow flex-col gap-8 p-10">
					<div className="flex grow flex-col items-center justify-center">
						<Outlet />
					</div>
					<Progress />
				</div>
				<div className="flex justify-center p-4">
					<p className="text-xs text-ink-dull opacity-50">
						&copy; {new Date().getFullYear()} Spacedrive Technology Inc.
					</p>
				</div>
				<div className="absolute -z-10">
					<div className="relative h-screen w-screen">
						<img src={BloomOne} className="absolute size-[2000px]" />
						{/* <img src={BloomThree} className="absolute w-[2000px] h-[2000px] -right-[200px]" /> */}
					</div>
				</div>
				{debugState.enabled && <DebugPopover />}
			</div>
		</OnboardingContext.Provider>
	);
};
