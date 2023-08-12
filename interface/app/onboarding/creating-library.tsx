import { Loader } from '@sd/ui';
import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './Layout';
import { useUnlockOnboardingScreen } from './Progress';

export default function OnboardingCreatingLibrary() {
	useUnlockOnboardingScreen();

	return (
		<OnboardingContainer>
			<span className="text-6xl">🛠</span>
			<OnboardingTitle>Creating your library</OnboardingTitle>
			<OnboardingDescription>Creating your library...</OnboardingDescription>
			<Loader className="mt-5" />
		</OnboardingContainer>
	);
}
