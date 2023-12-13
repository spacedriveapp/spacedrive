import { ArrowsClockwise, Cloud, Database, Factory } from '@phosphor-icons/react';
import { LibraryContextProvider, useClientContext, useFeatureFlag } from '@sd/client';

import { EphemeralSection } from './EphemeralSection';
import Icon from './Icon';
import { LibrarySection } from './LibrarySection';
import SidebarLink from './Link';
import Section from './Section';

export default () => {
	const { library } = useClientContext();

	const debugRoutes = useFeatureFlag('debugRoutes');

	return (
		<div className="no-scrollbar mask-fade-out flex grow flex-col space-y-5 overflow-x-hidden overflow-y-scroll pb-10">
			{/* <SidebarLink to="spacedrop">
					<Icon component={Broadcast} />
					Spacedrop
				</SidebarLink> */}
			{/*
				{/* <SidebarLink to="imports">
					<Icon component={ArchiveBox} />
					Imports
				</SidebarLink> */}
			{debugRoutes && (
				<Section name="Debug">
					<div className="space-y-0.5">
						<SidebarLink to="debug/sync">
							<Icon component={ArrowsClockwise} />
							Sync
						</SidebarLink>
						<SidebarLink to="debug/cloud">
							<Icon component={Cloud} />
							Cloud
						</SidebarLink>
						<SidebarLink to="debug/cache">
							<Icon component={Database} />
							Cache
						</SidebarLink>
						<SidebarLink to="debug/actors">
							<Icon component={Factory} />
							Actors
						</SidebarLink>
					</div>
				</Section>
			)}
			<EphemeralSection />
			{library && (
				<LibraryContextProvider library={library}>
					<LibrarySection />
				</LibraryContextProvider>
			)}
			{/* <Section name="Tools" actionArea={<SubtleButton />}>
				<SidebarLink disabled to="duplicate-finder">
					<Icon component={CopySimple} />
					Duplicates
				</SidebarLink>
				<SidebarLink disabled to="lost-and-found">
					<Icon component={Crosshair} />
					Find a File
				</SidebarLink>
				<SidebarLink disabled to="cache-cleaner">
					<Icon component={Eraser} />
					Cache Cleaner
				</SidebarLink>
				<SidebarLink disabled to="media-encoder">
					<Icon component={FilmStrip} />
					Media Encoder
				</SidebarLink>
			</Section> */}
			<div className="grow" />
		</div>
	);
};
