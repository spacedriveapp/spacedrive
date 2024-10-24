import { AlphaBg, AlphaBg_Light, AppLogo } from '@sd/assets/images';
import { Discord } from '@sd/assets/svgs/brands';
import { Button, ButtonLink } from '@sd/ui';
import { useIsDark, useLocale } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { OnboardingContainer } from './components';

export default function OnboardingPreRelease() {
	const platform = usePlatform();
	const isDark = useIsDark();

	const { t } = useLocale();

	return (
		<OnboardingContainer>
			<div className="relative w-screen text-center">
				<img
					src={isDark ? AlphaBg : AlphaBg_Light}
					alt="Spacedrive"
					className="absolute top-[-50px] z-0 w-full"
				/>
				<div className="relative z-10 flex flex-col gap-5">
					<div className="mb-5 flex w-full items-center justify-center gap-2">
						<img src={AppLogo} alt="" className="size-8" />
						<h1 className="font-plex text-[25px] font-semibold">Spacedrive</h1>
					</div>
					<h1 className="text-[40px] font-bold">{t('prelease_title')}</h1>
					<p className="mx-auto w-full max-w-[450px] text-sm text-ink-faint">
						{t('prerelease_description')}
					</p>
					<div className="mt-0 flex w-full items-center justify-center gap-2">
						<Button
							onClick={() => platform.openLink('https://discord.gg/gTaF2Z44f5')}
							className="flex gap-2"
							variant="gray"
						>
							<Discord className="size-4 fill-ink" />
							{t('join_discord')}
						</Button>
						<ButtonLink to="../new-library" replace variant="accent">
							{t('continue')}
						</ButtonLink>
					</div>
				</div>
			</div>
		</OnboardingContainer>
	);
}
