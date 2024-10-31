import { Globe } from '@phosphor-icons/react';
import { AppLogo } from '@sd/assets/images';
import { Discord, Github } from '@sd/assets/svgs/brands';
import { useBridgeQuery, useDebugStateEnabler } from '@sd/client';
import { Button, Divider } from '@sd/ui';
import { useLocale } from '~/hooks';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';
import { usePlatform } from '~/util/Platform';

export const Component = () => {
	const buildInfo = useBridgeQuery(['buildInfo']);
	const platform = usePlatform();
	const os = useOperatingSystem();
	const currentPlatformNiceName =
		os === 'browser' ? 'Web' : os == 'macOS' ? os : os.charAt(0).toUpperCase() + os.slice(1);
	const onClick = useDebugStateEnabler();

	const { t } = useLocale();

	return (
		<div className="h-auto">
			<div className="flex flex-row items-center">
				<img
					src={AppLogo}
					className="mr-8 size-[88px]"
					draggable="false"
					onClick={onClick}
				/>
				<div className="flex flex-col">
					<h1 className="text-2xl font-bold">
						Spacedrive {os !== 'unknown' && <>for {currentPlatformNiceName}</>}
					</h1>
					<span className="mt-1 text-sm text-ink-dull">
						The file manager from the future.
					</span>
					<span className="mt-1 text-xs text-ink-faint/80">
						v{buildInfo.data?.version || '-.-.-'} - {buildInfo.data?.commit || 'dev'}
					</span>
				</div>
			</div>
			<div className="my-5 flex gap-2">
				<Button
					onClick={() => {
						platform.openLink('https://discord.gg/ukRnWSnAbG');
					}}
					className="flex w-fit gap-2"
					variant="gray"
				>
					<Discord className="size-4 fill-ink" />
					{t('join_discord')}
				</Button>
				<Button
					href="https://github.com/spacedriveapp/spacedrive"
					target="_blank"
					className="flex w-fit gap-2"
					variant="accent"
				>
					<Github className="size-4 fill-white" />
					{t('star_on_github')}
				</Button>
				<Button
					onClick={() => {
						platform.openLink('https://spacedrive.app');
					}}
					className="flex w-fit gap-1"
					variant="accent"
				>
					<Globe className="size-4" />
					{t('website')}
				</Button>
			</div>
			<Divider />
			<div className="my-5">
				<h1 className="mb-3 font-plex text-lg font-bold text-ink">
					{t('about_vision_title')}
				</h1>
				<p className="w-full text-sm text-ink-faint">{t('about_vision_text')}</p>
			</div>
			<Divider />
			<div>
				<h1 className="my-5 font-plex text-lg font-bold text-ink">
					{t('meet_contributors_behind_spacedrive')}
				</h1>
				<img
					src="https://contrib.rocks/image?repo=spacedriveapp/spacedrive&columns=12&anon=1"
					draggable="false"
				/>
			</div>
		</div>
	);
};
