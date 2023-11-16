import { Button } from '@sd/ui';
import { Icon } from '~/components';
import { usePlatform } from '~/util/Platform';

import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './components';

export default function OnboardingFullDisk() {
	const { openDiskPermissions } = usePlatform();
	return (
		<OnboardingContainer>
			<Icon name="HDD" size={80} />
			<OnboardingTitle>Full disk permissions</OnboardingTitle>
			<OnboardingDescription>
				To provide the best experience, we need access to your disk in order to index your
				files. Your files are only available to you.
			</OnboardingDescription>
			<Button
				onClick={() => {
					openDiskPermissions();
				}}
				variant="gray"
				size="sm"
				className="mt-4"
			>
				Enable access
			</Button>
			<Button variant="accent" size="sm" className="mt-12">
				Continue
			</Button>
		</OnboardingContainer>
	);
}
