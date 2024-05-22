import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router';
import { Button } from '@sd/ui';
import { Icon } from '~/components';
import { useLocale } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './components';

export const FullDisk = () => {
	const [fdaVideo, setFdaVideo] = useState<string | null>(null);
	const { requestFdaMacos } = usePlatform();
	const navigate = useNavigate();

	const { t } = useLocale();

	useEffect(() => {
		import('@sd/assets/videos/Fda.mp4').then(
			(module) => {
				setFdaVideo(module.default);
			},
			(err) => {
				console.error(err);
				navigate('../locations', { replace: true });
			}
		);
	});

	return (
		<OnboardingContainer>
			<Icon name="HDD" size={80} />
			<OnboardingTitle>{t('full_disk_access')}</OnboardingTitle>
			<OnboardingDescription>{t('full_disk_access_description')}</OnboardingDescription>
			<div className="mt-5 w-full max-w-[450px]">
				{fdaVideo && (
					<video
						className="rounded-md"
						autoPlay
						loop
						muted
						controls={false}
						src={fdaVideo}
					/>
				)}
			</div>
			<div className="flex items-center gap-3">
				<Button onClick={requestFdaMacos} variant="gray" size="sm" className="my-5">
					{t('open_settings')}
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
					{t('continue')}
				</Button>
			</div>
		</OnboardingContainer>
	);
};
