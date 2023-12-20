import {
	ArrowsClockwise,
	Clock,
	Cloud,
	Database,
	Factory,
	Heart,
	Planet,
	Tag
} from '@phosphor-icons/react';
import {
	LibraryContextProvider,
	useClientContext,
	useFeatureFlag,
	useLibraryQuery
} from '@sd/client';

import { EphemeralSection } from './EphemeralSection';
import Icon from './Icon';
import { LibrarySection } from './LibrarySection';
import SidebarLink from './Link';
import Section from './Section';

export const COUNT_STYLE = `absolute right-1 min-w-[20px] top-1 flex h-[19px] px-1 items-center justify-center rounded-full border border-app-button/40 text-[9px]`;
export default () => {
	const { library } = useClientContext();

	const debugRoutes = useFeatureFlag('debugRoutes');

	const labelCount = useLibraryQuery(['labels.count']);

	return (
		<div className="no-scrollbar mask-fade-out flex grow flex-col space-y-5 overflow-x-hidden overflow-y-scroll pb-10">
			<div className="space-y-0.5">
				<SidebarLink to="overview">
					<Icon component={Planet} />
					Overview
				</SidebarLink>
				<SidebarLink to="recents">
					<Icon component={Clock} />
					Recents
					{/* <div className={COUNT_STYLE}>34</div> */}
				</SidebarLink>
				<SidebarLink to="favorites">
					<Icon component={Heart} />
					Favorites
					{/* <div className={COUNT_STYLE}>2</div> */}
				</SidebarLink>
				<SidebarLink to="labels">
					<Icon component={Tag} />
					Labels
					<div className={COUNT_STYLE}>{labelCount.data || 0}</div>
				</SidebarLink>
			</div>
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
