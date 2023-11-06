import { Info, Question } from '@phosphor-icons/react';
import { Button, Form, RadioGroupField } from '@sd/ui';
import { usePlatform } from '~/util/Platform';

import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './components';
import { shareTelemetry, useOnboardingContext } from './context';

export default function OnboardingPrivacy() {
	const { forms, submit } = useOnboardingContext();
	const platform = usePlatform();

	const form = forms.useForm('privacy');

	return (
		<Form
			form={form}
			onSubmit={form.handleSubmit(submit)}
			className="flex flex-col items-center"
		>
			<OnboardingContainer>
				<OnboardingTitle>Your Privacy</OnboardingTitle>
				<OnboardingDescription>
					Spacedrive is built for privacy, that's why we're open source and local first.
					So we'll make it very clear what data is shared with us.
				</OnboardingDescription>
				<div className="m-6">
					<RadioGroupField.Root {...form.register('shareTelemetry')}>
						{shareTelemetry.options.map(({ value, heading, description }) => (
							<RadioGroupField.Item key={value} value={value}>
								<h1 className="font-bold">{heading}</h1>
								<p className="text-sm text-ink-faint">{description}</p>
							</RadioGroupField.Item>
						))}
					</RadioGroupField.Root>
					<Button
						size="sm"
						className="mx-auto mt-5 flex items-center justify-center gap-1 text-center"
						variant="gray"
						onClick={() => {
							platform.openLink(
								'https://www.spacedrive.com/docs/product/resources/privacy'
							);
						}}
					>
						<Info size={13} />
						More info
					</Button>
				</div>
				<Button type="submit" className="mt-5 text-center" variant="accent" size="sm">
					Continue
				</Button>
			</OnboardingContainer>
		</Form>
	);
}
