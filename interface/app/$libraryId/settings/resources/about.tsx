import { AppLogo } from '@sd/assets/images';
import { Discord, Github } from '@sd/assets/svgs/brands';
import { Globe } from 'phosphor-react';
import { useBridgeQuery } from '@sd/client';
import { Button, Divider } from '@sd/ui';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';
import { usePlatform } from '~/util/Platform';

export const Component = () => {
	const buildInfo = useBridgeQuery(['buildInfo']);
	const platform = usePlatform();
	const os = useOperatingSystem();
	const currentPlatformNiceName =
		os === 'browser' ? 'Web' : os == 'macOS' ? os : os.charAt(0).toUpperCase() + os.slice(1);

	return (
		<div className="h-auto">
			<div className="flex flex-row items-center">
				<img src={AppLogo} className="mr-8 h-[88px] w-[88px]" />
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
					<Discord className="h-4 w-4 fill-ink" />
					Join Discord
				</Button>
				<Button
					href="https://github.com/spacedriveapp/spacedrive"
					target="_blank"
					className="flex w-fit gap-2"
					variant="accent"
				>
					<Github className="h-4 w-4 fill-white" />
					Star on GitHub
				</Button>
				<Button
					onClick={() => {
						platform.openLink('https://spacedrive.app');
					}}
					className="flex w-fit gap-1"
					variant="accent"
				>
					<Globe className="h-4 w-4 fill-ink" />
					Website
				</Button>
			</div>
			<Divider />
			<div className="my-5">
				<h1 className="mb-3 text-lg font-bold text-ink">Vision</h1>
				<p className="w-full text-sm text-ink-faint">
					Many of us have multiple cloud accounts, drives that aren’t backed up and data
					at risk of loss. We depend on cloud services like Google Photos and iCloud, but
					are locked in with limited capacity and almost zero interoperability between
					services and operating systems. Photo albums shouldn’t be stuck in a device
					ecosystem, or harvested for advertising data. They should be OS agnostic,
					permanent and personally owned. Data we create is our legacy, that will long
					outlive us—open source technology is the only way to ensure we retain absolute
					control over the data that defines our lives, at unlimited scale.
				</p>
			</div>
			<Divider />
			<div className="mb-20">
				<h1 className="my-5 text-lg font-bold text-ink">
					We also would like to thank all our contributors
				</h1>
				<img src="https://contrib.rocks/image?repo=spacedriveapp/spacedrive&columns=12" />
			</div>
		</div>
	);
};
