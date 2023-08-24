import { ArrowsClockwise, CopySimple, Crosshair, Eraser, FilmStrip, Planet } from 'phosphor-react';
import { LibraryContextProvider, useClientContext, useFeatureFlag } from '@sd/client';
import { SubtleButton } from '~/components/SubtleButton';
import { EphemeralSection } from './EphemeralSection';
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
				{/* <SidebarLink to="spacedrop">
					<Icon component={Broadcast} />
					Spacedrop
				</SidebarLink>
				<SidebarLink to="imports">
					<Icon component={ArchiveBox} />
					Imports
				</SidebarLink> */}
				{useFeatureFlag('syncRoute') && (
					<SidebarLink to="sync">
						<Icon component={ArrowsClockwise} />
						Sync
					</SidebarLink>
				)}
			</div>
			<EphemeralSection />
			{library && (
				<LibraryContextProvider library={library}>
					<LibrarySection />
				</LibraryContextProvider>
			)}
			<Section name="Tools" actionArea={<SubtleButton />}>
				<SidebarLink disabled to="duplicate-finder">
					<Icon component={CopySimple} />
					Duplicate Finder
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
			</Section>
			<div className="grow" />
		</div>
	);
};
