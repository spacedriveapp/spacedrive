import { EjectSimple } from '@phosphor-icons/react';
import { Drive, Globe, HDD, Home, SD } from '@sd/assets/icons';
import clsx from 'clsx';
import { useState } from 'react';
import { useBridgeQuery } from '@sd/client';
import { Button, tw } from '@sd/ui';
import { usePlatform } from '~/util/Platform';

import SidebarLink from './Link';
import Section from './Section';
import SeeMore from './SeeMore';

const SidebarIcon = tw.img`mr-1 h-5 w-5`;
const Name = tw.span`truncate`;

const EjectButton = ({ className }: { className?: string }) => (
	<Button className={clsx('absolute right-[2px] !p-[5px]', className)} variant="subtle">
		<EjectSimple weight="fill" size={18} className="h-3 w-3 opacity-70" />
	</Button>
);

export const EphemeralSection = () => {
	const [home, setHome] = useState<string | null>(null);

	const platform = usePlatform();
	platform.userHomeDir?.().then(setHome);

	const volumes = useBridgeQuery(['volumes.list']).data ?? [];

	const items = [
		{ type: 'network' },
		home ? { type: 'home', path: home } : null,
		...volumes.flatMap((volume, volumeIndex) =>
			volume.mount_points.map((mountPoint, index) =>
				mountPoint !== home
					? { type: 'volume', volume, mountPoint, volumeIndex, index }
					: null
			)
		)
	].filter(Boolean) as Array<{
		type: string;
		path?: string;
		volume?: any;
		mountPoint?: string;
		volumeIndex?: number;
		index?: number;
	}>;

	return home == null && volumes.length < 1 ? null : (
		<>
			<Section name="Local">
				<SeeMore
					items={items}
					renderItem={(item, index) => {
						if (item?.type === 'network') {
							return (
								<SidebarLink
									className="group relative w-full"
									to={`network/34`}
									key={index}
								>
									<SidebarIcon src={Globe} />
									<Name>Network</Name>
								</SidebarLink>
							);
						}

						if (item?.type === 'home') {
							return (
								<SidebarLink
									to={`ephemeral/0?path=${item.path}`}
									className="group relative w-full border border-transparent"
									key={index}
								>
									<SidebarIcon src={Home} />
									<Name>Home</Name>
								</SidebarLink>
							);
						}

						if (item?.type === 'volume') {
							const key = `${item.volumeIndex}-${item.index}`;
							const name =
								item.mountPoint === '/'
									? 'Root'
									: item.index === 0
									? item.volume.name
									: item.mountPoint;

							return (
								<SidebarLink
									to={`ephemeral/${key}?path=${item.mountPoint}`}
									key={key}
									className="group relative w-full border border-transparent"
								>
									<SidebarIcon
										src={
											item.volume.file_system === 'exfat'
												? SD
												: item.volume.name === 'Macintosh HD'
												? HDD
												: Drive
										}
									/>
									<Name>{name}</Name>
									{item.volume.disk_type === 'Removable' && <EjectButton />}
								</SidebarLink>
							);
						}

						return null; // This should never be reached, but is here to satisfy TypeScript
					}}
				/>
			</Section>
		</>
	);
};
