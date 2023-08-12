import { AlphaBg, AlphaBg_Light, AppLogo } from '@sd/assets/images';
import { Discord } from '@sd/assets/svgs/brands';
import { Button, ButtonLink } from '@sd/ui';
import { useIsDark } from '~/hooks';
import { usePlatform } from '~/util/Platform';
import { OnboardingContainer } from './Layout';

export default function OnboardingAlpha() {
	const platform = usePlatform();
	const isDark = useIsDark();

	return (
		<OnboardingContainer>
			<div className="relative w-screen text-center">
				<img
					src={isDark ? AlphaBg : AlphaBg_Light}
					alt="Alpha Background"
					className="absolute top-[-50px] z-0 w-full"
				/>
				<div className="relative z-10 flex flex-col gap-5">
					<div className="mb-5 flex w-full items-center justify-center gap-2">
						<img src={AppLogo} alt="Spacedrive" className="h-8 w-8" />
						<h1 className="text-[25px] font-semibold">Spacedrive</h1>
					</div>
					<h1 className="text-[40px] font-bold">Alpha Release</h1>
					<p className="mx-auto w-full max-w-[450px] text-sm text-ink-faint">
						We are delighted for you to try Spacedrive, now in Alpha release, showcasing
						exciting new features. As with any initial release, this version may contain
						some bugs. We kindly request your assistance in reporting any issues you
						encounter on our Discord channel. Your valuable feedback will greatly
						contribute to enhancing the user experience.
					</p>
					<div className="mt-0 flex w-full items-center justify-center gap-2">
						<Button
							onClick={() => platform.openLink('https://discord.gg/ukRnWSnAbG')}
							className="flex gap-2"
							variant="gray"
						>
							<Discord className="h-4 w-4 fill-ink" />
							Join Discord
						</Button>
						<ButtonLink to="../new-library" replace variant="accent">
							Continue
						</ButtonLink>
					</div>
				</div>
			</div>
		</OnboardingContainer>
	);
}
