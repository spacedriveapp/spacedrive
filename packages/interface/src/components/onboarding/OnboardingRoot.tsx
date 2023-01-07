import BloomOne from '@sd/assets/images/bloom-one.png';
import BloomThree from '@sd/assets/images/bloom-three.png';
import clsx from 'clsx';
import { Dispatch, SetStateAction, createContext, useState } from 'react';
import { Outlet } from 'react-router';

import { useOperatingSystem } from '../../hooks/useOperatingSystem';
import OnboardingProgress from './OnboardingProgress';

export const UnlockedScreens = createContext<{
	unlockedScreens: number[];
	unlockScreen?: (screenIndex: number) => void;
}>({ unlockedScreens: [] });

export default function OnboardingRoot() {
	const os = useOperatingSystem();
	const [unlockedScreens, setUnlockedScreens] = useState<number[]>([]);

	function unlockScreen(index: number) {
		setUnlockedScreens((prev) => {
			if (prev.includes(index)) return prev;
			return [...prev, index];
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
				<UnlockedScreens.Provider value={{ unlockedScreens, unlockScreen }}>
					<div className="flex flex-col items-center justify-center flex-grow">
						<Outlet />
					</div>
					<OnboardingProgress />
				</UnlockedScreens.Provider>
			</div>
			<div className="flex justify-center p-4">
				<p className="text-xs opacity-50 text-ink-dull">&copy; 2022 Spacedrive Technology Inc.</p>
			</div>
			<div className="absolute -z-10">
				<div className="relative w-screen h-screen">
					<img src={BloomOne} className="absolute w-[2000px] h-[2000px]" />
					<img src={BloomThree} className="absolute w-[2000px] h-[2000px] -right-[200px]" />
				</div>
			</div>
		</div>
	);
}

const macOnly = (platform: string | undefined, classnames: string) =>
	platform === 'macOS' ? classnames : '';
