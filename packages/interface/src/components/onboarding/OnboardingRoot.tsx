import BloomOne from '@sd/assets/images/bloom-one.png';
import BloomThree from '@sd/assets/images/bloom-three.png';
import { tw } from '@sd/ui';
import clsx from 'clsx';
import { Dispatch, SetStateAction, createContext, useState } from 'react';
import { Outlet, useLocation } from 'react-router';

import { useOperatingSystem } from '../../hooks/useOperatingSystem';
import OnboardingProgress from './OnboardingProgress';
import { useCurrentOnboardingScreenKey } from './helpers/screens';

export const OnboardingStateContext = createContext<{
	unlockedScreens: string[];
	unlockScreen?: (screenIndex: string) => void;
}>({ unlockedScreens: [] });

export default function OnboardingRoot() {
	const os = useOperatingSystem();
	const [unlockedScreens, setUnlockedScreens] = useState<string[]>([]);

	const currentScreenKey = useCurrentOnboardingScreenKey();

	function unlockScreen(key: string) {
		setUnlockedScreens((prev) => {
			if (prev.includes(key)) return prev;
			return [...prev, key];
		});
	}

	return (
		<div
			className={clsx(
				macOnly(os, 'bg-opacity-[0.75]'),
				'flex flex-col h-screen bg-sidebar text-ink'
			)}
		>
			<div data-tauri-drag-region className="z-50 flex flex-shrink-0 w-full h-9" />

			<div className="flex flex-col flex-grow p-10 -mt-5">
				<OnboardingStateContext.Provider value={{ unlockedScreens, unlockScreen }}>
					<div className="flex flex-col items-center justify-center flex-grow">
						<Outlet />
					</div>
					<OnboardingProgress />
				</OnboardingStateContext.Provider>
			</div>
			<div className="flex justify-center p-4">
				<p className="text-xs opacity-50 text-ink-dull">&copy; 2022 Spacedrive Technology Inc.</p>
			</div>
			<div className="absolute -z-10">
				<div className="relative w-screen h-screen">
					<img src={BloomOne} className="absolute w-[2000px] h-[2000px]" />
					{/* <img src={BloomThree} className="absolute w-[2000px] h-[2000px] -right-[200px]" /> */}
				</div>
			</div>
		</div>
	);
}

const macOnly = (platform: string | undefined, classnames: string) =>
	platform === 'macOS' ? classnames : '';

export const OnboardingContainer = tw.div`flex flex-col items-center`;
export const OnboardingTitle = tw.h2`mb-2 text-3xl font-bold`;
export const OnboardingDescription = tw.p`max-w-xl text-center text-ink-dull`;
export const OnboardingImg = tw.img`w-20 h-20 mb-2`;
