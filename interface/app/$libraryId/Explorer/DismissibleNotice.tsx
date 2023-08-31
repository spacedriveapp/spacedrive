import {
	Collection,
	Collection_Light,
	Image,
	Image_Light,
	Video,
	Video_Light
} from '@sd/assets/icons';
import { ReactNode } from 'react';
import { ExplorerLayout } from '@sd/client';
import DismissibleNotice from '~/components/DismissibleNotice';
import { useIsDark } from '~/hooks';
import { dismissibleNoticeStore } from '~/hooks/useDismissibleNoticeStore';
import { useExplorerContext } from './Context';
import { useExplorerStore } from './store';

const MediaViewIcon = () => {
	const isDark = useIsDark();

	return (
		<div className="relative ml-3 mr-10 h-14 w-14 shrink-0">
			<img
				src={isDark ? Image : Image_Light}
				className="absolute -top-1 left-6 h-14 w-14 rotate-6 overflow-hidden"
			/>
			<img
				src={isDark ? Video : Video_Light}
				className="absolute top-2 z-10 h-14 w-14 -rotate-6 overflow-hidden"
			/>
		</div>
	);
};

const CollectionIcon = () => {
	const isDark = useIsDark();

	return (
		<div className="ml-3 mr-4 h-14 w-14 shrink-0">
			<img src={isDark ? Collection : Collection_Light} />
		</div>
	);
};

interface Notice {
	key: keyof typeof dismissibleNoticeStore;
	title: string;
	description: string;
	icon: ReactNode;
}

const notices = {
	grid: {
		key: 'gridView',
		title: 'Grid View',
		description:
			"Get a visual overview of your files with Grid View. This view displays your files and folders as thumbnail images, making it easy to quickly identify the file you're looking for.",
		icon: <CollectionIcon />
	},
	list: {
		key: 'listView',
		title: 'List View',
		description:
			'Easily navigate through your files and folders with List View. This view displays your files in a simple, organized list format, allowing you to quickly locate and access the files you need.',
		icon: <CollectionIcon />
	},
	media: {
		key: 'mediaView',
		title: 'Media View',
		description:
			'Discover photos and videos easily, Media View will show results starting at the current location including sub directories.',
		icon: <MediaViewIcon />
	}
	// columns: undefined
} satisfies Record<ExplorerLayout, Notice | undefined>;

export default () => {
	const settings = useExplorerContext().useSettingsSnapshot();

	const notice = notices[settings.layoutMode];

	if (!notice) return null;

	return (
		<DismissibleNotice
			title={
				<>
					<span className="font-normal">Meet</span> {notice.title}
				</>
			}
			icon={notice.icon}
			description={notice.description}
			className="m-5"
			storageKey={notice.key}
			onContextMenu={(e) => e.preventDefault()}
		/>
	);
};
