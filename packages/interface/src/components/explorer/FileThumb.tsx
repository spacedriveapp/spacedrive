import Archive from '@sd/assets/images/Archive.png';
import Compressed from '@sd/assets/images/Compressed.png';
import DocumentPdf from '@sd/assets/images/Document_pdf.png';
import Encrypted from '@sd/assets/images/Encrypted.png';
import Executable from '@sd/assets/images/Executable.png';
import File from '@sd/assets/images/File.png';
import Video from '@sd/assets/images/Video.png';
import clsx from 'clsx';
import { ExplorerItem, ObjectKind, isObject, isPath } from '@sd/client';
import { useExplorerStore } from '~/hooks/useExplorerStore';
import { usePlatform } from '~/util/Platform';
import { Folder } from '../icons/Folder';

export function getExplorerItemData(data: ExplorerItem) {
	const objectData = data ? (isObject(data) ? data.item : data.item.object) : null;

	return {
		cas_id: (isObject(data) ? data.item.file_paths[0]?.cas_id : data.item.cas_id) || null,
		isDir: isPath(data) && data.item.is_dir,
		kind: ObjectKind[objectData?.kind || 0] || null,
		hasThumbnail: data.has_thumbnail,
		extension: data.item.extension
	};
}

// const icons = import.meta.glob('../../../../assets/icons/*.svg');
interface FileItemProps {
	data: ExplorerItem;
	size: number;
	className?: string;
}
export function FileThumb({ data, size }: FileItemProps) {
	const { cas_id, isDir, kind, hasThumbnail, extension } = getExplorerItemData(data);
	return (
		<div
			className={clsx(
				'relative flex h-full shrink-0 items-center justify-center rounded border-2 border-transparent p-1'
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
						'border-app-line max-h-full w-auto max-w-full overflow-hidden rounded-sm border-2 object-cover shadow shadow-black/40',
					kind === 'Video' && 'rounded border-x-0 border-y-[7px] !border-black'
				)}
			/>
			{extension && kind === 'Video' && (
				<div className="absolute bottom-4 right-2 rounded bg-black/60 py-0.5 px-1 text-[9px] font-semibold uppercase opacity-70">
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
	imgStyle?: string;
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

	if (url && hasThumbnail)
		return (
			<img
				style={imgStyle}
				decoding="async"
				className={clsx('z-90 pointer-events-none', imgClassName)}
				src={url}
			/>
		);

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
