import { EjectSimple } from '@phosphor-icons/react';
import { Drive, Globe, HDD, Home, SD } from '@sd/assets/icons';
import clsx from 'clsx';
import { useState } from 'react';
import { useBridgeQuery } from '@sd/client';
import { Button, tw } from '@sd/ui';
import { usePlatform } from '~/util/Platform';

import SidebarLink from './Link';
import Section from './Section';

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

	return home == null && volumes.length < 1 ? null : (
		<>
			<Section name="Local">
				<SidebarLink className="group relative w-full" to={`network/34`}>
					<SidebarIcon src={Globe} />
					<Name>Network</Name>
				</SidebarLink>
				{home && (
					<SidebarLink
						to={`ephemeral/0?path=${home}`}
						className="group relative w-full border border-transparent"
					>
						<SidebarIcon src={Home} />
						<Name>Home</Name>
					</SidebarLink>
				)}
				{volumes.map((volume, volumeIndex) => {
					const mountPoints = volume.mount_points;
					mountPoints.sort((a, b) => a.length - b.length);
					return mountPoints.map((mountPoint, index) => {
						const key = `${volumeIndex}-${index}`;
						if (mountPoint == home) return null;

						const name =
							mountPoint === '/' ? 'Root' : index === 0 ? volume.name : mountPoint;
						return (
							<SidebarLink
								to={`ephemeral/${key}?path=${mountPoint}`}
								key={key}
								className="group relative w-full border border-transparent"
							>
								<SidebarIcon
									src={
										volume.file_system === 'exfat'
											? SD
											: volume.name === 'Macintosh HD'
											? HDD
											: Drive
									}
								/>

								<Name>{name}</Name>
								{volume.disk_type === 'Removable' && <EjectButton />}
							</SidebarLink>
						);
					});
				})}
			</Section>
		</>
	);
};
