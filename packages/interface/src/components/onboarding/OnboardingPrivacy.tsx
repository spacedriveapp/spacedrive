import { Button, RadioGroup } from '@sd/ui';
import { useNavigate } from 'react-router';

import { useUnlockOnboardingScreen } from './OnboardingProgress';
import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './OnboardingRoot';

export default function OnboardingPrivacy() {
	const navigate = useNavigate();

	useUnlockOnboardingScreen();

	return (
		<OnboardingContainer>
			<OnboardingTitle>Your Privacy</OnboardingTitle>
			<OnboardingDescription>
				Spacedrive is built for privacy, that's why we're open source and local first. So we'll make
				it very clear what data is shared with us.
			</OnboardingDescription>
			<div className="m-4">
				<RadioGroup.Root defaultValue="1">
					<RadioGroup.Item value="1">
						<h1 className="font-bold">Share anonymous usage</h1>
						<p className="text-sm text-ink-faint">
							A short description about option one. I wonder if it still looks goo when it is
							long...
						</p>
					</RadioGroup.Item>
					<RadioGroup.Item value="2">
						<h1 className="font-bold">Share nothing</h1>
						<p className="text-sm text-ink-faint">
							A short description about option one. I wonder if it still looks goo when it is
							long...
						</p>
					</RadioGroup.Item>
					{/* <RadioGroup.Item value="3">Option 3</RadioGroup.Item> */}
				</RadioGroup.Root>
			</div>
			<Button variant="accent" size="sm">
				Continue
			</Button>
		</OnboardingContainer>
	);
}
