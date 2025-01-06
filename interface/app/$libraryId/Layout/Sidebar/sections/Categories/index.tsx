import {
	Aperture,
	Cube,
	CubeFocus,
	FrameCorners,
	Image,
	MonitorPlay,
	Person,
	Record
} from '@phosphor-icons/react';
import { HardwareModel, usePeers } from '@sd/client';
import { Icon } from '~/components';
import { useLocale } from '~/hooks';
import { hardwareModelToIcon } from '~/util/hardware';

import SidebarLink from '../../SidebarLayout/Link';
import Section from '../../SidebarLayout/Section';
import { SeeMore } from '../../SidebarLayout/SeeMore';

export default function CategoriesSection() {
	const { t } = useLocale();

	return (
		<Section name={t('Categories')}>
			<SeeMore>
				<SidebarLink to="/overview" id="screenshots" className="group relative w-full">
					<FrameCorners size={18} className="mr-1" />
					<span className="truncate">Screenshots</span>
				</SidebarLink>
				<SidebarLink to="/selfies" id="selfies" className="group relative w-full">
					<Person size={18} className="mr-1" />
					<span className="truncate">Selfies</span>
				</SidebarLink>
				<SidebarLink to="/videos" id="videos" className="group relative w-full">
					<MonitorPlay size={18} className="mr-1" />
					<span className="truncate">Videos</span>
				</SidebarLink>
				<SidebarLink to="/live-photos" id="live-photos" className="group relative w-full">
					<FrameCorners size={18} className="mr-1" />
					<span className="truncate">Live Photos</span>
				</SidebarLink>
				<SidebarLink
					to="/screen-recordings"
					id="screen-recordings"
					className="group relative w-full"
				>
					<Record size={18} className="mr-1" />
					<span className="truncate">Screen Recordings</span>
				</SidebarLink>
				<SidebarLink to="/spacial" id="spacial" className="group relative w-full">
					<CubeFocus size={18} className="mr-1" />
					<span className="truncate">Spacial</span>
				</SidebarLink>
				<SidebarLink to="/3d" id="3d" className="group relative w-full">
					<Cube size={18} className="mr-1" />
					<span className="truncate">3D Objects</span>
				</SidebarLink>
				<SidebarLink to="/spacial" id="spacial" className="group relative w-full">
					<Aperture size={18} className="mr-1" />
					<span className="truncate">RAW</span>
				</SidebarLink>
				{/* <SidebarLink to="/spacial" id="spacial" className="group relative w-full">
					<Aperture size={18} className="mr-1" />
					<span className="truncate">RAW</span>
				</SidebarLink> */}
			</SeeMore>
		</Section>
	);
}
