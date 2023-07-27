import { useState } from 'react';
import { useBridgeQuery } from '@sd/client';
import { Folder, SubtleButton } from '~/components';
import { usePlatform } from '~/util/Platform';
import SidebarLink from './Link';
import Section from './Section';

export const EphemeralSection = () => {
	const [home, setHome] = useState<string | null>(null);

	const platform = usePlatform();
	platform.userHomeDir?.().then(setHome);

	const { data: volumes } = useBridgeQuery(['volumes.list']);

	return (
		<>
			<Section name="Explore" actionArea={<SubtleButton />}>
				{home && (
					<SidebarLink
						to={`ephemeral/0?path=${home}`}
						className="group relative w-full border border-transparent"
					>
						<div className="relative -mt-0.5 mr-1 shrink-0 grow-0">
							<Folder size={18} />
						</div>

						<span className="truncate">Home</span>
					</SidebarLink>
				)}
				{volumes?.map((volume, index) => (
					<SidebarLink
						to={`ephemeral/${index + 1}?path=${volume.mount_points[0]}`}
						key={index}
						className="group relative w-full border border-transparent"
					>
						<div className="relative -mt-0.5 mr-1 shrink-0 grow-0">
							<Folder size={18} />
						</div>

						<span className="truncate">{volume.name}</span>
					</SidebarLink>
				))}
			</Section>
		</>
	);
};
