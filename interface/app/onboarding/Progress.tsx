import clsx from 'clsx';
import { useEffect } from 'react';
import { useLocation, useNavigate } from 'react-router';
import { getOnboardingStore, unlockOnboardingScreen, useOnboardingStore } from '@sd/client';
import routes from '.';

export const useCurrentOnboardingScreenKey = (): string | null => {
	const { pathname } = useLocation();

	if (pathname.startsWith(`/onboarding/`)) {
		return pathname.split('/')[2] || null;
	}

	return null;
};

// screens are locked to prevent users from skipping ahead
export function useUnlockOnboardingScreen() {
	const currentScreenKey = useCurrentOnboardingScreenKey()!;

	useEffect(() => {
		unlockOnboardingScreen(currentScreenKey, getOnboardingStore().unlockedScreens);
	}, [currentScreenKey]);
}

export default function OnboardingProgress() {
	const ob_store = useOnboardingStore();
	const navigate = useNavigate();
	const currentScreenKey = useCurrentOnboardingScreenKey();

	return (
		<div className="flex w-full items-center justify-center">
			<div className="flex items-center justify-center space-x-1">
				{routes.map(({ path }) => {
					if (!path) return null;

					return (
						<div
							key={path}
							onClick={() => {
								if (ob_store.unlockedScreens.includes(path)) {
									navigate(`/onboarding/${path}`);
								}
							}}
							className={clsx(
								'hover:bg-ink h-2 w-2 rounded-full transition',
								currentScreenKey === path ? 'bg-ink' : 'bg-ink-faint',
								!ob_store.unlockedScreens.includes(path) && 'opacity-10'
							)}
						/>
					);
				})}
			</div>
		</div>
	);
}
