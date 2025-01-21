import {
	ArrowsClockwise,
	Books,
	Cloud,
	Database,
	FlyingSaucer,
	GearSix,
	GlobeSimple,
	HardDrive,
	Key,
	KeyReturn,
	PaintBrush,
	PuzzlePiece,
	Receipt,
	ShareNetwork,
	ShieldCheck,
	TagSimple,
	User
} from '@phosphor-icons/react';
import clsx from 'clsx';
import { useFeatureFlag } from '@sd/client';
import { tw } from '@sd/ui';
import { useLocale, useOperatingSystem } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import Icon from '../Layout/Sidebar/SidebarLayout/Icon';
import SidebarLink from '../Layout/Sidebar/SidebarLayout/Link';
import { useLayoutStore } from '../Layout/store';
import { NavigationButtons } from '../TopBar/NavigationButtons';

const Heading = tw.div`mb-1 ml-1 text-xs font-semibold text-gray-400 font-plex tracking-wide`;
const Section = tw.div`space-y-0.5`;

export default () => {
	const os = useOperatingSystem();
	const { platform } = usePlatform();
	const { sidebar } = useLayoutStore();

	// const isPairingEnabled = useFeatureFlag('p2pPairing');
	const isBackupsEnabled = useFeatureFlag('backups');
	// const cloudSync = useFeatureFlag('cloudSync');

	const { t } = useLocale();

	return (
		<div className="custom-scroll no-scrollbar h-full w-60 max-w-[180px] shrink-0 border-r border-app-line/50 pb-5">
			{platform === 'tauri' ? (
				<div
					data-tauri-drag-region={os === 'macOS'}
					className={clsx(
						'mb-3 flex h-3 w-full p-3 pl-[14px] pt-[11px]',
						sidebar.collapsed && os === 'macOS' && 'justify-end'
					)}
				>
					{os !== 'windows' && <NavigationButtons />}
				</div>
			) : (
				<div className="h-3" />
			)}

			<div className="space-y-6 px-4 py-3">
				<Section>
					<Heading>{t('client')}</Heading>
					<SidebarLink to="client/general">
						<Icon component={GearSix} />
						{t('general')}
					</SidebarLink>
					{/* Disabling for now until sync is ready. */}
					{/* <SidebarLink to="client/account">
						<Icon component={User} />
						{t('account')}
					</SidebarLink> */}
					<SidebarLink to="node/libraries">
						<Icon component={Books} />
						{t('libraries')}
					</SidebarLink>
					<SidebarLink to="client/privacy">
						<Icon component={ShieldCheck} />
						{t('privacy')}
					</SidebarLink>
					<SidebarLink to="client/appearance">
						<Icon component={PaintBrush} />
						{t('appearance')}
					</SidebarLink>
					<SidebarLink to="client/network">
						<Icon component={GlobeSimple} />
						{t('network')}
					</SidebarLink>
					<SidebarLink to="client/backups" disabled={!isBackupsEnabled}>
						<Icon component={Database} />
						{t('backups')}
					</SidebarLink>
					<SidebarLink to="client/keybindings">
						<Icon component={KeyReturn} />
						{t('keybinds')}
					</SidebarLink>
					<SidebarLink to="client/extensions" disabled>
						<Icon component={PuzzlePiece} />
						{t('extensions')}
					</SidebarLink>
				</Section>
				<Section>
					<Heading>{t('library')}</Heading>
					<SidebarLink to="library/general">
						<Icon component={GearSix} />
						{t('general')}
					</SidebarLink>
					<SidebarLink to="library/volumes">
						<Icon component={ShareNetwork} />
						Volumes
					</SidebarLink>
					<SidebarLink to="library/sync">
						<Icon component={ArrowsClockwise} />
						{t('sync')}
					</SidebarLink>
					<SidebarLink to="library/locations">
						<Icon component={HardDrive} />
						{t('locations')}
					</SidebarLink>
					<SidebarLink to="library/tags">
						<Icon component={TagSimple} />
						{t('tags')}
					</SidebarLink>
					{/* <SidebarLink to="library/saved-searches">
						<Icon component={MagnifyingGlass} />
						Saved Searches
					</SidebarLink> */}

					<SidebarLink disabled to="library/clouds">
						<Icon component={Cloud} />
						{t('clouds')}
					</SidebarLink>
					<SidebarLink to="library/keys" disabled>
						<Icon component={Key} />
						{t('keys')}
					</SidebarLink>
				</Section>
				<Section>
					<Heading>{t('resources')}</Heading>
					<SidebarLink to="resources/about">
						<Icon component={FlyingSaucer} />
						{t('about')}
					</SidebarLink>
					<SidebarLink to="resources/changelog">
						<Icon component={Receipt} />
						{t('changelog')}
					</SidebarLink>
				</Section>
			</div>
		</div>
	);
};
