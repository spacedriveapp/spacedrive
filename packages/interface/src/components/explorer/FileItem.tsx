import { ExplorerItem, getExplorerStore } from '@sd/client';
import clsx from 'clsx';
import { HTMLAttributes } from 'react';

import FileThumb from './FileThumb';
import { isObject } from './utils';

interface Props extends HTMLAttributes<HTMLDivElement> {
	data: ExplorerItem;
	selected: boolean;
	index: number;
}

function FileItem({ data, selected, index, ...rest }: Props) {
	// const store = useExplorerStore();

	// store.layoutMode;

	// props.index === store.selectedRowIndex

	const isVid = isVideo(data.extension || '');

	return (
		<div
			onContextMenu={(e) => {
				const objectId = isObject(data) ? data.id : data.object?.id;
				if (objectId != undefined) {
					getExplorerStore().contextMenuObjectId = objectId;
					if (index != undefined) {
						getExplorerStore().selectedRowIndex = index;
					}
				}
			}}
			{...rest}
			draggable
			className={clsx('inline-block w-[100px] mb-3', rest.className)}
		>
			<div
				style={{ width: getExplorerStore().gridItemSize, height: getExplorerStore().gridItemSize }}
				className={clsx(
					'border-2 border-transparent rounded-lg text-center mb-1 active:translate-y-[1px]',
					{
						'bg-gray-50 dark:bg-gray-750': selected
					}
				)}
			>
				<div
					className={clsx(
						'flex relative items-center justify-center h-full  p-1 rounded border-transparent border-2 shrink-0'
					)}
				>
					<FileThumb
						className={clsx(
							'border-4 border-gray-250 rounded shadow-md shadow-gray-750 object-cover max-w-full max-h-full w-auto overflow-hidden',
							isVid && 'border-gray-950 border-x-0 border-y-[9px]'
						)}
						data={data}
						kind={data.extension === 'zip' ? 'zip' : isVid ? 'video' : 'other'}
						size={getExplorerStore().gridItemSize}
					/>
					{data?.extension && isVid && (
						<div className="absolute bottom-4 font-semibold opacity-70 right-2 py-0.5 px-1 text-[9px] uppercase bg-gray-800 rounded">
							{data.extension}
						</div>
					)}
				</div>
			</div>
			<div className="flex justify-center">
				<span
					className={clsx(
						'px-1.5 py-[1px] truncate text-center rounded-md text-xs font-medium text-gray-550 dark:text-gray-300 cursor-default ',
						{
							'bg-primary !text-white': selected
						}
					)}
				>
					{data?.name}
					{data?.extension && `.${data.extension}`}
				</span>
			</div>
		</div>
	);
}

export default FileItem;

function isVideo(extension: string) {
	return [
		'avi',
		'asf',
		'mpeg',
		'mts',
		'mpe',
		'vob',
		'qt',
		'mov',
		'asf',
		'asx',
		'mjpeg',
		'ts',
		'mxf',
		'm2ts',
		'f4v',
		'wm',
		'3gp',
		'm4v',
		'wmv',
		'mp4',
		'webm',
		'flv',
		'mpg',
		'hevc',
		'ogv',
		'swf',
		'wtv'
	].includes(extension);
}
