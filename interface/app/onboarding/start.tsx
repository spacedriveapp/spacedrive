import AppLogo from '@sd/assets/images/logo.png';
import { ButtonLink } from '@sd/ui';
import { OnboardingContainer, OnboardingDescription, OnboardingImg } from './Layout';

export default function OnboardingStart() {
	return (
		<OnboardingContainer>
			<OnboardingImg src={AppLogo} className="mb-8 h-36 w-36 shrink-0" />

			<h1 className="text-ink mb-2 text-center text-4xl font-bold">
				The file explorer from the future.
			</h1>
			<OnboardingDescription>
				Welcome to Spacedrive, an open source cross-platform file manager.
			</OnboardingDescription>
			<div className="mt-6 space-x-3">
				<ButtonLink to="/onboarding/new-library" variant="accent" size="md">
					Get started
				</ButtonLink>
			</div>
		</OnboardingContainer>
	);
}
