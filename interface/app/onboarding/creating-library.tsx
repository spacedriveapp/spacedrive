import { Loader } from '@sd/ui';
import { useLocale } from '~/hooks';

import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './components';

export default function OnboardingCreatingLibrary() {
	const { t } = useLocale();

	return (
		<OnboardingContainer>
			<span className="text-6xl">🛠</span>
			<OnboardingTitle>{t('creating_your_library')}</OnboardingTitle>
			<OnboardingDescription>{t('creating_your_library')}...</OnboardingDescription>
			<Loader className="mt-5" />
		</OnboardingContainer>
	);
}
