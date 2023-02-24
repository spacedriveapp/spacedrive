import Archive from '@sd/assets/images/Archive.png';
import Compressed from '@sd/assets/images/Compressed.png';
import DocumentPdf from '@sd/assets/images/Document_pdf.png';
import Encrypted from '@sd/assets/images/Encrypted.png';
import Executable from '@sd/assets/images/Executable.png';
import File from '@sd/assets/images/File.png';
import Video from '@sd/assets/images/Video.png';
import clsx from 'clsx';
import { ExplorerItem, isObject, isPath } from '@sd/client';
import { useExplorerStore } from '~/hooks/useExplorerStore';
import { usePlatform } from '~/util/Platform';
import { Folder } from '../icons/Folder';

interface Props {
	data: ExplorerItem;
	size: number;
	className?: string;
	style?: React.CSSProperties;
	iconClassNames?: string;
	kind?: string;
}

// const icons = import.meta.glob('../../../../assets/icons/*.svg');

export default function FileThumb({ data, ...props }: Props) {
	const platform = usePlatform();
	// const Icon = useMemo(() => {
	// 	const icon = icons[`../../../../assets/icons/${item.extension}.svg`];

	// 	const Icon = icon
	// 		? lazy(() => icon().then((v) => ({ default: (v as any).ReactComponent })))
	// 		: undefined;
	// 	return Icon;
	// }, [item.extension]);

	if (isPath(data) && data.item.is_dir) return <Folder size={props.size * 0.7} />;

	if (data.has_thumbnail) {
		const cas_id = isObject(data) ? data.item.file_paths[0]?.cas_id : data.item.cas_id;

		if (!cas_id) return <div></div>;

		const url = platform.getThumbnailUrlById(cas_id);

		if (url)
			return (
				<img
					style={props.style}
					decoding="async"
					// width={props.size}
					className={clsx('z-90 pointer-events-none', props.className)}
					src={url}
				/>
			);
	}

	let icon = File;
	// Hacky (and temporary) way to integrate thumbnails
	if (props.kind === 'Archive') icon = Archive;
	else if (props.kind === 'Video') icon = Video;
	else if (props.kind === 'Document' && data.item.extension === 'pdf') icon = DocumentPdf;
	else if (props.kind === 'Executable') icon = Executable;
	else if (props.kind === 'Encrypted') icon = Encrypted;
	else if (props.kind === 'Compressed') icon = Compressed;

	return <img src={icon} className={clsx('h-full overflow-hidden', props.iconClassNames)} />;
}
