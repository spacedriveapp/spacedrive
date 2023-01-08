import clsx from 'clsx';
import { useContext, useEffect, useState } from 'react';
import { useLocation, useNavigate } from 'react-router';

import { UnlockedScreens } from './OnboardingRoot';

const SCREEN_COUNT = 9;

// screens are locked to prevent users from skipping ahead
export function useOnboardingScreenMounted() {
	const { pathname } = useLocation();

	const { unlockScreen } = useContext(UnlockedScreens);

	const currentIndex = Number(pathname.split('/')[2]) || 0;

	useEffect(() => {
		if (unlockScreen) unlockScreen(currentIndex);
	}, [currentIndex, unlockScreen]);
}

export default function OnboardingProgress() {
	const { pathname } = useLocation();
	const { unlockedScreens } = useContext(UnlockedScreens);
	const navigate = useNavigate();
	const currentIndex = Number(pathname.split('/')[2]) || 0;

	return (
		<div className="flex items-center justify-center w-full">
			<div className="flex items-center justify-center space-x-1">
				{Array.from({ length: SCREEN_COUNT }).map((_, index) => (
					<div
						key={index}
						onClick={() => {
							if (unlockedScreens.includes(index)) {
								navigate(`/onboarding/${index}`);
							}
						}}
						className={clsx(
							'w-2 h-2 rounded-full hover:bg-ink transition',
							currentIndex === index ? 'bg-ink' : 'bg-ink-faint',
							!unlockedScreens.includes(index) && 'opacity-10'
						)}
					/>
				))}
			</div>
		</div>
	);
}
