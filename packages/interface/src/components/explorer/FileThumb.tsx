import Archive from '@sd/assets/images/Archive.png';
import Compressed from '@sd/assets/images/Compressed.png';
import DocumentPdf from '@sd/assets/images/Document_pdf.png';
import Encrypted from '@sd/assets/images/Encrypted.png';
import Executable from '@sd/assets/images/Executable.png';
import File from '@sd/assets/images/File.png';
import Video from '@sd/assets/images/Video.png';
import clsx from 'clsx';
import { CSSProperties } from 'react';
import { ExplorerItem, ObjectKind, isObject, isPath } from '@sd/client';
import { usePlatform } from '~/util/Platform';
import { Folder } from '../icons/Folder';
import { getExplorerItemData } from './util';

// const icons = import.meta.glob('../../../../assets/icons/*.svg');
interface FileItemProps {
	data: ExplorerItem;
	size: number;
	className?: string;
}

export function FileThumb({ data, size, className }: FileItemProps) {
	const { cas_id, isDir, kind, hasThumbnail, extension } = getExplorerItemData(data);

	// 10 percent of the size
	const videoBarsHeight = Math.floor(size / 10);

	// calculate 16:9 ratio for height from size
	const videoHeight = Math.floor((size * 9) / 16) + videoBarsHeight * 2;

	return (
		<div
			className={clsx(
				'relative flex h-full shrink-0 items-center justify-center border-2 border-transparent',
				className
			)}
		>
			<FileThumbImg
				size={size}
				hasThumbnail={hasThumbnail}
				isDir={isDir}
				cas_id={cas_id}
				extension={extension}
				kind={kind}
				imgClassName={clsx(
					hasThumbnail &&
						'max-h-full w-auto max-w-full rounded-sm object-cover shadow shadow-black/30',
					kind === 'Image' && size > 60 && 'border-app-line border-2',
					kind === 'Video' && 'rounded border-x-0 !border-black'
				)}
				imgStyle={
					kind === 'Video'
						? {
								borderTopWidth: videoBarsHeight,
								borderBottomWidth: videoBarsHeight,
								width: size,
								height: videoHeight
						  }
						: {}
				}
			/>
			{extension && kind === 'Video' && size > 80 && (
				<div className="absolute bottom-[22%] right-2 rounded bg-black/60 py-0.5 px-1 text-[9px] font-semibold uppercase opacity-70">
					{extension}
				</div>
			)}
		</div>
	);
}
interface FileThumbImgProps {
	isDir: boolean;
	cas_id: string | null;
	kind: string | null;
	extension: string | null;
	size: number;
	hasThumbnail: boolean;
	imgClassName?: string;
	imgStyle?: CSSProperties;
}

export function FileThumbImg({
	isDir,
	cas_id,
	kind,
	size,
	hasThumbnail,
	extension,
	imgClassName,
	imgStyle
}: FileThumbImgProps) {
	const platform = usePlatform();

	if (isDir) return <Folder size={size * 0.7} />;

	if (!cas_id) return <div></div>;
	const url = platform.getThumbnailUrlById(cas_id);

	if (url && hasThumbnail) {
		return (
			<img
				style={{ ...imgStyle, maxWidth: size, width: size - 10 }}
				decoding="async"
				className={clsx('z-90 pointer-events-none bg-black', imgClassName)}
				src={url}
			/>
		);
	}

	let icon = File;
	// Hacky (and temporary) way to integrate thumbnails
	if (kind === 'Archive') icon = Archive;
	else if (kind === 'Video') icon = Video;
	else if (kind === 'Document' && extension === 'pdf') icon = DocumentPdf;
	else if (kind === 'Executable') icon = Executable;
	else if (kind === 'Encrypted') icon = Encrypted;
	else if (kind === 'Compressed') icon = Compressed;

	return <img src={icon} className={clsx('h-full overflow-hidden')} />;
}
