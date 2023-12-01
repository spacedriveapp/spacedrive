import { ArrowsClockwise, Cloud, Planet } from '@phosphor-icons/react';
import { useNavigate } from 'react-router';
import { LibraryContextProvider, useClientContext, useFeatureFlag } from '@sd/client';
import { Tooltip } from '@sd/ui';
import { useKeysMatcher, useShortcut } from '~/hooks';

import { EphemeralSection } from './EphemeralSection';
import Icon from './Icon';
import { LibrarySection } from './LibrarySection';
import SidebarLink from './Link';

export default () => {
	const { library } = useClientContext();
	const navigate = useNavigate();
	const symbols = useKeysMatcher(['Meta', 'Shift']);

	// useShortcut('navToOverview', (e) => {
	// 	e.stopPropagation();
	// 	navigate('overview');
	// });

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
			<div className="space-y-0.5">
				{useFeatureFlag('syncRoute') && (
					<SidebarLink to="sync">
						<Icon component={ArrowsClockwise} />
						Sync
					</SidebarLink>
				)}
				{useFeatureFlag('cloud') && (
					<SidebarLink to="cloud">
						<Icon component={Cloud} />
						Cloud
					</SidebarLink>
				)}
			</div>
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
