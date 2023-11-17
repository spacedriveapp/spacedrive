import { Fda } from '@sd/assets/videos';
import { motion } from 'framer-motion';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import { Button } from '@sd/ui';
import { Icon } from '~/components';
import { usePlatform } from '~/util/Platform';

import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './components';

export default function OnboardingFullDisk() {
	const { requestFdaMacos } = usePlatform();
	const [showVideo, setShowVideo] = useState(false);
	const navigate = useNavigate();
	return (
		<OnboardingContainer>
			<Icon name="HDD" size={80} />
			<OnboardingTitle>Full disk access</OnboardingTitle>
			<OnboardingDescription>
				To provide the best experience, we need access to your disk in order to index your
				files. Your files are only available to you.
			</OnboardingDescription>
			{!showVideo ? (
				<>
					<div className="flex items-center gap-3">
						<Button onClick={requestFdaMacos} variant="gray" size="sm" className="my-5">
							Enable access
						</Button>
						<Button onClick={() => setShowVideo((t) => !t)} variant="outline">
							How to enable
						</Button>
					</div>
				</>
			) : (
				<motion.div
					initial={{ opacity: 0 }}
					animate={{ opacity: 1 }}
					transition={{ duration: 0.5, ease: 'easeInOut' }}
					className="mt-5 w-full max-w-[450px]"
				>
					<video className="rounded-md" autoPlay loop muted controls={false} src={Fda} />
				</motion.div>
			)}
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
				{showVideo && (
					<Button
						onClick={() => setShowVideo((t) => !t)}
						variant="gray"
						size="sm"
						className="mt-8"
					>
						Close
					</Button>
				)}
			</div>
		</OnboardingContainer>
	);
}
