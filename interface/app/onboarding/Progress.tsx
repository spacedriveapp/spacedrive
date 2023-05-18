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
	const obStore = useOnboardingStore();
	const navigate = useNavigate();
	const currentScreenKey = useCurrentOnboardingScreenKey();

	return (
		<div className="flex w-full items-center justify-center">
			<div className="flex items-center justify-center space-x-1">
				{routes.map(({ path }) => {
					if (!path) return null;

					return (
						<button
							key={path}
							disabled={!obStore.unlockedScreens.includes(path)}
							onClick={() => navigate(`/onboarding/${path}`, { replace: true })}
							className={clsx(
								'h-2 w-2 rounded-full transition hover:bg-ink disabled:opacity-10',
								currentScreenKey === path ? 'bg-ink' : 'bg-ink-faint'
							)}
						/>
					);
				})}
			</div>
		</div>
	);
}
