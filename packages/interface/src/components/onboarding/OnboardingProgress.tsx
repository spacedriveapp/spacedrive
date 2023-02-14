import clsx from 'clsx';
import { useEffect } from 'react';
import { useNavigate } from 'react-router';
import { getOnboardingStore, unlockOnboardingScreen, useOnboardingStore } from '@sd/client';
import { ONBOARDING_SCREENS } from './OnboardingRoot';
import { useCurrentOnboardingScreenKey } from './helpers/screens';

// screens are locked to prevent users from skipping ahead
export function useUnlockOnboardingScreen() {
	const currentScreenKey = useCurrentOnboardingScreenKey()!;

	const ob_store = getOnboardingStore();

	useEffect(() => {
		unlockOnboardingScreen(currentScreenKey, ob_store.unlockedScreens);
	}, [currentScreenKey]);
}

export default function OnboardingProgress() {
	const ob_store = useOnboardingStore();
	const navigate = useNavigate();
	const currentScreenKey = useCurrentOnboardingScreenKey();

	return (
		<div className="flex items-center justify-center w-full">
			<div className="flex items-center justify-center space-x-1">
				{ONBOARDING_SCREENS.map(({ isSkippable, key }) => (
					<div
						key={key}
						onClick={() => {
							if (ob_store.unlockedScreens.includes(key)) {
								navigate(`/onboarding/${key}`);
							}
						}}
						className={clsx(
							'w-2 h-2 rounded-full hover:bg-ink transition',
							currentScreenKey === key ? 'bg-ink' : 'bg-ink-faint',
							!ob_store.unlockedScreens.includes(key) && 'opacity-10'
						)}
					/>
				))}
			</div>
		</div>
	);
}
