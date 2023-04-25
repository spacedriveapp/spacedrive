import { Collection, Image, Video } from '@sd/assets/icons';
import clsx from 'clsx';
import DismissibleNotice from '~/components/DismissibleNotice';

const MediaViewIcon = () => (
	<div className="relative mr-10 ml-3 h-14 w-14 flex-shrink-0">
		<img src={Image} className="absolute left-6 -top-1 h-14 w-14 rotate-6 overflow-hidden" />
		<img src={Video} className="absolute top-2 z-10 h-14 w-14 -rotate-6 overflow-hidden" />
	</div>
);

const CollectionIcon = () => (
	<div className="mr-4 ml-3 h-14 w-14 flex-shrink-0">
		<img src={Collection} />
	</div>
);

const notices = {
	gridView: {
		title: 'Grid View',
		description: 'Get a visual overview of your files with Grid View. This view displays your files and folders as thumbnail images, making it easy to quickly identify the file you're looking for.',
		icon: <CollectionIcon />
	},
	listView: {
		title: 'List View',
		description: 'Easily navigate through your files and folders with List View. This view displays your files in a simple, organized list format, allowing you to quickly locate and access the files you need.',
		icon: <CollectionIcon />
	},
	mediaView: {
		title: 'Media View',
		description:
			'Discover photos and videos easily, Media View will show results starting at the current location including sub directories.',
		icon: <MediaViewIcon />
	}
};

interface Props {
	notice: keyof typeof notices;
	className?: string;
}

export default (props: Props) => {
	const notice = notices[props.notice];

	return (
		<DismissibleNotice
			title={
				<>
					<span className="font-normal">Meet</span> {notice.title}
				</>
			}
			icon={notice.icon}
			description={notice.description}
			className={clsx('m-5', props.className)}
			storageKey={props.notice}
		/>
	);
};
