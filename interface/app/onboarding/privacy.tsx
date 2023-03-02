import { useNavigate } from 'react-router';
import { getOnboardingStore } from '@sd/client';
import { Button } from '@sd/ui';
import { Form, RadioGroup, useZodForm, z } from '@sd/ui/src/forms';
import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './Layout';
import { useUnlockOnboardingScreen } from './Progress';

const shareTelemetry = RadioGroup.options([
	z.literal('share-telemetry'),
	z.literal('no-telemetry')
]).details({
	'share-telemetry': {
		heading: 'Share anonymous usage',
		description: 'Share completely anonymous telemetry data to help the developers improve the app'
	},
	'no-telemetry': {
		heading: 'Share nothing',
		description: 'Do not share any telemetry data with the developers'
	}
});

const schema = z.object({
	shareTelemetry: shareTelemetry.schema
});

export default function OnboardingPrivacy() {
	const navigate = useNavigate();

	useUnlockOnboardingScreen();

	const form = useZodForm({
		schema,
		defaultValues: {
			shareTelemetry: 'share-telemetry'
		}
	});

	const onSubmit = form.handleSubmit(async (data) => {
		getOnboardingStore().shareTelemetryDataWithDevelopers =
			data.shareTelemetry === 'share-telemetry';

		navigate('/onboarding/creating-library');
	});

	return (
		<Form form={form} onSubmit={onSubmit} className="flex flex-col items-center">
			<OnboardingContainer>
				<OnboardingTitle>Your Privacy</OnboardingTitle>
				<OnboardingDescription>
					Spacedrive is built for privacy, that's why we're open source and local first. So we'll
					make it very clear what data is shared with us.
				</OnboardingDescription>
				<div className="m-4">
					<RadioGroup.Root {...form.register('shareTelemetry')}>
						{shareTelemetry.options.map(({ value, heading, description }) => (
							<RadioGroup.Item key={value} value={value}>
								<h1 className="font-bold">{heading}</h1>
								<p className="text-ink-faint text-sm">{description}</p>
							</RadioGroup.Item>
						))}
					</RadioGroup.Root>
				</div>
				<Button className="text-center" type="submit" variant="accent" size="sm">
					Continue
				</Button>
			</OnboardingContainer>
		</Form>
	);
}
