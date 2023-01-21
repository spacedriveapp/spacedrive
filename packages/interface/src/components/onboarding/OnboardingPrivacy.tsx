import { getOnboardingStore } from '@sd/client';
import { Button, RadioGroup, forms } from '@sd/ui';
import { Controller } from 'react-hook-form';
import { useNavigate } from 'react-router';

import { useUnlockOnboardingScreen } from './OnboardingProgress';
import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './OnboardingRoot';

const { Input, z, useZodForm, Form } = forms;

const schema = z.object({
	shareTelemetryDataWithDevelopers: z.string()
});

export default function OnboardingPrivacy() {
	const navigate = useNavigate();

	useUnlockOnboardingScreen();

	const form = useZodForm({
		schema,
		defaultValues: {
			shareTelemetryDataWithDevelopers: 'share-telemetry'
		}
	});

	console.log({ formState: form.formState });

	const _onSubmit = form.handleSubmit(async (data) => {
		switch (data.shareTelemetryDataWithDevelopers) {
			case 'share-telemetry':
				getOnboardingStore().shareTelemetryDataWithDevelopers = true;
				break;
			case 'no-telemetry':
				getOnboardingStore().shareTelemetryDataWithDevelopers = false;
				break;
		}
		navigate('/onboarding/creating-library');
		return;
	});

	return (
		<Form form={form} onSubmit={_onSubmit}>
			<OnboardingContainer>
				<OnboardingTitle>Your Privacy</OnboardingTitle>
				<OnboardingDescription>
					Spacedrive is built for privacy, that's why we're open source and local first. So we'll
					make it very clear what data is shared with us.
				</OnboardingDescription>
				<div className="m-4">
					<Controller
						control={form.control}
						name="shareTelemetryDataWithDevelopers"
						render={({ field }) => (
							<RadioGroup.Root
								onValueChange={field.onChange}
								value={field.value}
								defaultValue="share-telemetry"
							>
								<RadioGroup.Item value="share-telemetry">
									<h1 className="font-bold">Share anonymous usage</h1>
									<p className="text-sm text-ink-faint">
										Share completely anonymous telemetry data to help the developers improve the app
									</p>
								</RadioGroup.Item>
								<RadioGroup.Item value="no-telemetry">
									<h1 className="font-bold">Share nothing</h1>
									<p className="text-sm text-ink-faint">
										Do not share any telemetry data with the developers
									</p>
								</RadioGroup.Item>
							</RadioGroup.Root>
						)}
					/>
				</div>
				<Button type="submit" variant="accent" size="sm">
					Continue
				</Button>
			</OnboardingContainer>
		</Form>
	);
}
