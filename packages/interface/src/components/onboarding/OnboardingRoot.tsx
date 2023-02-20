import BloomOne from '@sd/assets/images/bloom-one.png';
import clsx from 'clsx';
import { ComponentType, useEffect } from 'react';
import { Outlet, useNavigate } from 'react-router';
import { getOnboardingStore } from '@sd/client';
import { tw } from '@sd/ui';
import { useOperatingSystem } from '../../hooks/useOperatingSystem';
import OnboardingCreatingLibrary from './OnboardingCreatingLibrary';
import OnboardingMasterPassword from './OnboardingMasterPassword';
import OnboardingNewLibrary from './OnboardingNewLibrary';
import OnboardingPrivacy from './OnboardingPrivacy';
import OnboardingProgress from './OnboardingProgress';
import OnboardingStart from './OnboardingStart';

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
		component: OnboardingMasterPassword,
		key: 'master-password'
	},
	{
		component: OnboardingPrivacy,
		key: 'privacy'
	},
	{
		component: OnboardingCreatingLibrary,
		key: 'creating-library'
	}
];

export const OnboardingContainer = tw.div`flex flex-col items-center`;
export const OnboardingTitle = tw.h2`mb-2 text-3xl font-bold`;
export const OnboardingDescription = tw.p`max-w-xl text-center text-ink-dull`;
export const OnboardingImg = tw.img`w-20 h-20 mb-2`;

export default function OnboardingRoot() {
	const os = useOperatingSystem();
	const navigate = useNavigate();
	const ob_store = getOnboardingStore();

	useEffect(() => {
		// This is neat because restores the last active screen, but only if it is not the starting screen
		// Ignoring if people navigate back to the start if progress has been made
		if (ob_store.unlockedScreens.length > 1) {
			navigate(`/onboarding/${ob_store.lastActiveScreen}`);
		}
	}, []);

	return (
		<div
			className={clsx(
				macOnly(os, 'bg-opacity-[0.75]'),
				'bg-sidebar text-ink flex h-screen flex-col'
			)}
		>
			<div data-tauri-drag-region className="z-50 flex h-9 w-full shrink-0" />

			<div className="-mt-5 flex grow flex-col p-10">
				<div className="flex grow flex-col items-center justify-center">
					<Outlet />
				</div>
				<OnboardingProgress />
			</div>
			<div className="flex justify-center p-4">
				<p className="text-ink-dull text-xs opacity-50">&copy; 2022 Spacedrive Technology Inc.</p>
			</div>
			<div className="absolute -z-10">
				<div className="relative h-screen w-screen">
					<img src={BloomOne} className="absolute h-[2000px] w-[2000px]" />
					{/* <img src={BloomThree} className="absolute w-[2000px] h-[2000px] -right-[200px]" /> */}
				</div>
			</div>
		</div>
	);
}

const macOnly = (platform: string | undefined, classnames: string) =>
	platform === 'macOS' ? classnames : '';
