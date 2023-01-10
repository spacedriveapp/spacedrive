import clsx from 'clsx';
import { type ComponentType, useContext, useEffect } from 'react';
import { useLocation, useNavigate } from 'react-router';

import OnboardingNewLibrary from './OnboardingNewLibrary';
import OnboardingPrivacy from './OnboardingPrivacy';
import { OnboardingStateContext } from './OnboardingRoot';
import OnboardingStart from './OnboardingStart';
import { useCurrentOnboardingScreenKey } from './helpers/screens';

interface OnboardingScreen {
	/**
	 * React component for rendering this screen.
	 */
	component: ComponentType<Record<string, never>>;
	/**
	 * Unique key used to record progression to this screen
	 */
	key: string;
	/**
	 * Sets whether the user is allowed to skip this screen.
	 * @default false
	 */
	isSkippable?: boolean;
}

export const ONBOARDING_SCREENS: OnboardingScreen[] = [
	{
		component: OnboardingStart,
		key: 'start'
	},
	{
		component: OnboardingNewLibrary,
		key: 'new-library'
	},
	{
		component: OnboardingPrivacy,
		key: 'privacy'
	},
	{
		component: OnboardingNewLibrary,
		key: 'jeff2'
	},
	{
		component: OnboardingNewLibrary,
		key: 'jeff3'
	}
];

// screens are locked to prevent users from skipping ahead
export function useUnlockOnboardingScreen() {
	const currentScreenKey = useCurrentOnboardingScreenKey()!;

	const { unlockScreen } = useContext(OnboardingStateContext);

	useEffect(() => {
		if (unlockScreen) unlockScreen(currentScreenKey);
	}, [currentScreenKey, unlockScreen]);
}

export default function OnboardingProgress() {
	const { unlockedScreens } = useContext(OnboardingStateContext);
	const navigate = useNavigate();
	const currentScreenKey = useCurrentOnboardingScreenKey();

	return (
		<div className="flex items-center justify-center w-full">
			<div className="flex items-center justify-center space-x-1">
				{ONBOARDING_SCREENS.map(({ isSkippable, key }) => (
					<div
						key={key}
						onClick={() => {
							if (unlockedScreens.includes(key)) {
								navigate(`/onboarding/${key}`);
							}
						}}
						className={clsx(
							'w-2 h-2 rounded-full hover:bg-ink transition',
							currentScreenKey === key ? 'bg-ink' : 'bg-ink-faint',
							!unlockedScreens.includes(key) && 'opacity-10'
						)}
					/>
				))}
			</div>
		</div>
	);
}
