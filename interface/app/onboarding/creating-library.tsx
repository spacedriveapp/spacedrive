import { Loader } from '@sd/ui';
import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './Layout';

export default function OnboardingCreatingLibrary() {
	return (
		<OnboardingContainer>
			<span className="text-6xl">🛠</span>
			<OnboardingTitle>Creating your library</OnboardingTitle>
			<OnboardingDescription>Creating your library...</OnboardingDescription>
			<Loader className="mt-5" />
		</OnboardingContainer>
	);
}
