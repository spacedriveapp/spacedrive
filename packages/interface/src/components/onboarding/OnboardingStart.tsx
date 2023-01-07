import AppLogo from '@sd/assets/images/logo.png';
import { Button, ButtonLink } from '@sd/ui';
import { useState } from 'react';
import { useLocation, useNavigate } from 'react-router';

import CreateLibraryDialog from '../dialog/CreateLibraryDialog';
import { useOnboardingScreenMounted } from './OnboardingProgress';

export default function OnboardingStart() {
	const navigate = useNavigate();

	useOnboardingScreenMounted();

	return (
		<>
			<img src={AppLogo} className="w-32 h-32 mb-8" />

			<h1 className="mb-2 text-4xl font-bold text-center text-ink">
				The file explorer from the future.
			</h1>
			<p className="text-center text-ink-faint">
				Welcome to Spacedrive, create a library to get started.
			</p>
			<div className="mt-6 space-x-3">
				<Button onClick={() => navigate('/onboarding/1')} variant="accent" size="md">
					Get started
				</Button>
			</div>
		</>
	);
}
