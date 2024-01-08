import { fda } from '@sd/assets/videos';
import { useNavigate } from 'react-router';
import { Button } from '@sd/ui';
import { Icon } from '~/components';
import { usePlatform } from '~/util/Platform';

import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './components';

export const FullDisk = () => {
	const { requestFdaMacos } = usePlatform();
	const navigate = useNavigate();

	return (
		<OnboardingContainer>
			<Icon name="HDD" size={80} />
			<OnboardingTitle>Full disk access</OnboardingTitle>
			<OnboardingDescription>
				To provide the best experience, we need access to your disk in order to index your
				files. Your files are only available to you.
			</OnboardingDescription>
			<div className="mt-5 w-full max-w-[450px]">
				<video className="rounded-md" autoPlay loop muted controls={false} src={fda} />
			</div>
			<div className="flex items-center gap-3">
				<Button onClick={requestFdaMacos} variant="gray" size="sm" className="my-5">
					Open Settings
				</Button>
			</div>
			<div className="flex gap-3">
				<Button
					onClick={() => {
						navigate('../locations', { replace: true });
					}}
					variant="accent"
					size="sm"
					className="mt-8"
				>
					Continue
				</Button>
			</div>
		</OnboardingContainer>
	);
};
