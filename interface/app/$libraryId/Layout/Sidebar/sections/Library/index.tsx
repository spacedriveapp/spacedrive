import { Clock, Heart, Planet, Tag } from '@phosphor-icons/react';
import { useLibraryQuery } from '@sd/client';

import Icon from '../../SidebarLayout/Icon';
import SidebarLink from '../../SidebarLayout/Link';

export const COUNT_STYLE = `absolute right-1 min-w-[20px] top-1 flex h-[19px] px-1 items-center justify-center rounded-full border border-app-button/40 text-[9px]`;

export default function LibrarySection() {
	const labelCount = useLibraryQuery(['labels.count']);

	return (
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
			<SidebarLink disabled to="labels">
				<Icon component={Tag} />
				Labels
				<div className={COUNT_STYLE}>{labelCount.data || 0}</div>
			</SidebarLink>
		</div>
	);
}
