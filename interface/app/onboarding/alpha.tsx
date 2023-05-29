import { AlphaBg, AppLogo } from '@sd/assets/images';
import { Discord } from '@sd/assets/svgs/brands';
import { useNavigate } from 'react-router-dom';
import { Button } from '@sd/ui';
import { usePlatform } from '~/util/Platform';
import { OnboardingContainer } from './Layout';

export default function OnboardingAlpha() {
	const navigate = useNavigate();
	const platform = usePlatform();
	return (
		<OnboardingContainer>
			<img src={AlphaBg} alt="Alpha Background" className="absolute top-[120px] z-0" />
			<div className="z-1 relative mx-auto mt-14 w-full max-w-[450px] text-center">
				<div className="mb-5 flex w-full items-center justify-center gap-2">
					<img src={AppLogo} alt="Spacedrive" className="h-8 w-8" />
					<h1 className="text-[25px] font-semibold">Spacedrive</h1>
				</div>
				<h1 className="text-[40px] font-bold">Alpha Release</h1>
				<p className="mt-3 text-sm text-ink-faint">
					We are delighted to announce the release of Spacedrive's alpha version,
					showcasing exciting new features. As with any initial release, this version may
					contain some bugs. We cannot guarantee that your data will stay intact. We
					kindly request your assistance in reporting any issues you encounter on our
					Discord channel. Your valuable feedback will greatly contribute to enhancing the
					user experience.
				</p>
				<div className="mt-10 flex w-full items-center justify-center gap-2">
					<Button
						onClick={() => {
							platform.openLink('https://discord.gg/3QWVWJ7');
						}}
						className="flex gap-2"
						variant="gray"
					>
						<Discord className="h-5 w-5 fill-white" />
						Join Discord
					</Button>
					<Button
						onClick={() => {
							navigate('/onboarding/start', { replace: true });
						}}
						variant="accent"
					>
						Continue
					</Button>
				</div>
			</div>
		</OnboardingContainer>
	);
}
