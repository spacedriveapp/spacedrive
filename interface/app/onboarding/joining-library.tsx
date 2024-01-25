import { Loader } from '@sd/ui';

import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './components';

export default function OnboardingCreatingLibrary() {
	return (
		<OnboardingContainer>
			<span className="text-6xl">🛠</span>
			<OnboardingTitle>Joining library</OnboardingTitle>
			<OnboardingDescription>Joing library...</OnboardingDescription>
			<Loader className="mt-5" />
		</OnboardingContainer>
	);
}
