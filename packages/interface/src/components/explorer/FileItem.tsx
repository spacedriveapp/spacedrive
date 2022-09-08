import { explorerStore } from '@sd/client';
import { ExplorerItem } from '@sd/core';
import clsx from 'clsx';
import { HTMLAttributes } from 'react';
import { useSnapshot } from 'valtio';

import FileThumb from './FileThumb';
import { isObject } from './utils';

interface Props extends HTMLAttributes<HTMLDivElement> {
	data: ExplorerItem;
	selected: boolean;
	index: number;
}

function FileItem(props: Props) {
	const store = useSnapshot(explorerStore);

	return (
		<div
			onContextMenu={(e) => {
				const objectId = isObject(props.data) ? props.data.id : props.data.file?.id;
				if (objectId != undefined) {
					explorerStore.contextMenuObjectId = objectId;
					if (props.index != undefined) {
						explorerStore.selectedRowIndex = props.index;
					}
				}
			}}
			draggable
			{...props}
			className={clsx('inline-block w-[100px] mb-3', props.className)}
		>
			<div
				style={{ width: store.gridItemSize, height: store.gridItemSize }}
				className={clsx(
					'border-2 border-transparent rounded-lg text-center mb-1 active:translate-y-[1px]',
					{
						'bg-gray-50 dark:bg-gray-750': props.selected
					}
				)}
			>
				<div
					className={clsx(
						'relative grid place-content-center min-w-0 h-full p-1 rounded border-transparent border-2 shrink-0'
					)}
				>
					<FileThumb
						className={clsx(
							'border-4 border-gray-250 rounded-sm shadow-md shadow-gray-750 max-h-full max-w-full overflow-hidden'
						)}
						data={props.data}
						size={store.gridItemSize}
					/>
				</div>
			</div>
			<div className="flex justify-center">
				<span
					className={clsx(
						'px-1.5 py-[1px] truncate text-center rounded-md text-xs font-medium text-gray-550 dark:text-gray-300 cursor-default',
						{
							'bg-primary !text-white': props.selected
						}
					)}
				>
					{props.data?.name}.{props.data?.extension}
				</span>
			</div>
		</div>
	);
}

export default FileItem;
