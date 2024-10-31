import clsx from 'clsx';
import { useEffect } from 'react';
import { useMatch, useNavigate } from 'react-router';
import { onboardingStore, unlockOnboardingScreen, useOnboardingStore } from '@sd/client';
import { useOperatingSystem } from '~/hooks';

export default function OnboardingProgress() {
	const obStore = useOnboardingStore();
	const navigate = useNavigate();
	const os = useOperatingSystem();

	const match = useMatch('/onboarding/:screen');

	const currentScreen = match?.params?.screen;

	useEffect(() => {
		if (!currentScreen) return;

		unlockOnboardingScreen(currentScreen, onboardingStore.unlockedScreens);
	}, [currentScreen]);

	const routes = [
		'prerelease',
		'new-library',
		os === 'macOS' && 'full-disk',
		'locations',
		'privacy',
		'creating-library'
	].filter(Boolean);

	return (
		<div className="flex w-full items-center justify-center">
			<div className="flex items-center justify-center space-x-1">
				{routes.map((path) => {
					if (!path) return null;

					return (
						<button
							key={path}
							disabled={!obStore.unlockedScreens.includes(path)}
							onClick={() => navigate(path, { replace: true })}
							className={clsx(
								'size-2 rounded-full transition hover:bg-ink disabled:opacity-10',
								currentScreen === path ? 'bg-ink' : 'bg-ink-faint'
							)}
						/>
					);
				})}
			</div>
		</div>
	);
}
