import archive from '@sd/assets/images/Archive.png';
import documentPdf from '@sd/assets/images/Document_pdf.png';
import executable from '@sd/assets/images/Executable.png';
import file from '@sd/assets/images/File.png';
import video from '@sd/assets/images/Video.png';
import clsx from 'clsx';
import { Suspense, lazy, useMemo } from 'react';
import { ExplorerItem } from '@sd/client';
import { useExplorerStore } from '~/hooks/useExplorerStore';
import { usePlatform } from '~/util/Platform';
import { Folder } from '../icons/Folder';
import { isObject, isPath } from './utils';

interface Props {
	data: ExplorerItem;
	size: number;
	className?: string;
	style?: React.CSSProperties;
	iconClassNames?: string;
	kind?: string;
}

const icons = import.meta.glob('../../../../assets/icons/*.svg');

export default function FileThumb({ data, ...props }: Props) {
	const platform = usePlatform();
	const store = useExplorerStore();

	const Icon = useMemo(() => {
		const icon = icons[`../../../../assets/icons/${data.extension as any}.svg`];

		const Icon = icon
			? lazy(() => icon().then((v) => ({ default: (v as any).ReactComponent })))
			: undefined;
		return Icon;
	}, [data.extension]);

	if (isPath(data) && data.is_dir) return <Folder size={props.size * 0.7} />;

	const cas_id = isObject(data) ? data.cas_id : data.object?.cas_id;

	if (!cas_id) return <div></div>;

	const has_thumbnail = isObject(data)
		? data.has_thumbnail
		: isPath(data)
		? data.object?.has_thumbnail
		: !!store.newThumbnails[cas_id];

	const url = platform.getThumbnailUrlById(cas_id);

	if (has_thumbnail && url)
		return (
			<img
				style={props.style}
				decoding="async"
				// width={props.size}
				className={clsx('pointer-events-none z-90', props.className)}
				src={url}
			/>
		);

	let icon = file;
	// Hacky (and temporary) way to integrate thumbnails
	if (props.kind === 'Archive') icon = archive;
	else if (props.kind === 'Video') icon = video;
	else if (props.kind === 'Document' && data.extension === 'pdf') icon = documentPdf;
	else if (props.kind === 'Executable') icon = executable;
	else if (props.kind === 'Encrypted') icon = archive;

	return <img src={icon} className={clsx('overflow-hidden h-full', props.iconClassNames)} />;
}
