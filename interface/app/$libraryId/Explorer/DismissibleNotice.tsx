import { ReactNode } from 'react';
import { ExplorerLayout } from '@sd/client';
import i18n from '~/app/I18n';
import { Icon } from '~/components';
import DismissibleNotice from '~/components/DismissibleNotice';
import { useLocale } from '~/hooks';
import { dismissibleNoticeStore } from '~/hooks/useDismissibleNoticeStore';

import { useExplorerContext } from './Context';

const MediaViewIcon = () => {
	return (
		<div className="relative ml-3 mr-10 size-14 shrink-0">
			<Icon
				name="Image"
				className="absolute -top-1 left-6 size-14 rotate-6 overflow-hidden"
			/>
			<Icon name="Video" className="absolute top-2 z-10 size-14 -rotate-6 overflow-hidden" />
		</div>
	);
};

const CollectionIcon = () => {
	return (
		<div className="ml-3 mr-4 size-14 shrink-0">
			<Icon name="Collection" />
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
		title: i18n.t('grid_view'),
		description: i18n.t('grid_view_notice_description'),
		icon: <CollectionIcon />
	},
	list: {
		key: 'listView',
		title: i18n.t('list_view'),
		description: i18n.t('list_view_notice_description'),
		icon: <CollectionIcon />
	},
	media: {
		key: 'mediaView',
		title: i18n.t('media_view'),
		description: i18n.t('media_view_notice_description'),
		icon: <MediaViewIcon />
	}
	// columns: undefined
} satisfies Record<ExplorerLayout, Notice | undefined>;

export default () => {
	const { t } = useLocale();

	const settings = useExplorerContext().useSettingsSnapshot();

	const notice = notices[settings.layoutMode];

	if (!notice) return null;

	return (
		<DismissibleNotice
			title={<span className="font-normal">{t('meet_title', { title: notice.title })}</span>}
			icon={notice.icon}
			description={notice.description}
			className="m-5"
			storageKey={notice.key}
			onContextMenu={(e) => e.preventDefault()}
		/>
	);
};
