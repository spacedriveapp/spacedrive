import {
	ArchiveBox,
	Broadcast,
	CirclesFour,
	CopySimple,
	Crosshair,
	Eraser,
	FilmStrip,
	MonitorPlay,
	Planet
} from 'phosphor-react';
import { useClientContext } from '@sd/client';
import { SubtleButton } from '~/components/SubtleButton';
import Icon from './Icon';
import { LibrarySection } from './LibrarySection';
import SidebarLink from './Link';
import Section from './Section';

export default () => {
	const { library } = useClientContext();

	return (
		<div className="no-scrollbar mask-fade-out flex grow flex-col overflow-x-hidden overflow-y-scroll pb-10">
			<div className="space-y-0.5">
				<SidebarLink to="overview">
					<Icon component={Planet} />
					Overview
				</SidebarLink>
				<SidebarLink to="spaces">
					<Icon component={CirclesFour} />
					Spaces
				</SidebarLink>
				{/* <SidebarLink to="people">
						<Icon component={UsersThree} />
						People
					</SidebarLink> */}
				<SidebarLink to="media">
					<Icon component={MonitorPlay} />
					Media
				</SidebarLink>
				<SidebarLink to="spacedrop">
					<Icon component={Broadcast} />
					Spacedrop
				</SidebarLink>
				<SidebarLink to="imports">
					<Icon component={ArchiveBox} />
					Imports
				</SidebarLink>
			</div>
			{library && <LibrarySection />}
			<Section name="Tools" actionArea={<SubtleButton />}>
				<SidebarLink to="duplicate-finder">
					<Icon component={CopySimple} />
					Duplicate Finder
				</SidebarLink>
				<SidebarLink to="lost-and-found">
					<Icon component={Crosshair} />
					Find a File
				</SidebarLink>
				<SidebarLink to="cache-cleaner">
					<Icon component={Eraser} />
					Cache Cleaner
				</SidebarLink>
				<SidebarLink to="media-encoder">
					<Icon component={FilmStrip} />
					Media Encoder
				</SidebarLink>
			</Section>
			<div className="grow" />
		</div>
	);
};
