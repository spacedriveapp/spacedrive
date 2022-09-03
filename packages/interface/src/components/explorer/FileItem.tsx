import { ReactComponent as Folder } from '@sd/assets/svgs/folder.svg';
import { LocationContext, useExplorerStore } from '@sd/client';
import { ExplorerData, ExplorerItem, File, FilePath } from '@sd/core';
import clsx from 'clsx';
import React, { useContext } from 'react';

import icons from '../../assets/icons';
import FileThumb from './FileThumb';
import { isObject, isPath } from './utils';

interface Props extends React.HTMLAttributes<HTMLDivElement> {
	data: ExplorerItem;
	selected: boolean;
	size: number;
	index: number;
}

export default function FileItem(props: Props) {
	const { set } = useExplorerStore();
	const size = props.size || 100;

	return (
		<div
			onContextMenu={(e) => {
				const objectId = isObject(props.data) ? props.data.id : props.data.file?.id;
				if (objectId != undefined) {
					set({ contextMenuObjectId: objectId });
					if (props.index != undefined) set({ selectedRowIndex: props.index });
				}
			}}
			draggable
			{...props}
			className={clsx('inline-block w-[100px] mb-3', props.className)}
		>
			<div
				style={{ width: size, height: size }}
				className={clsx(
					'border-2 border-transparent rounded-lg text-center mb-1 active:translate-y-[1px]',
					{
						'bg-gray-50 dark:bg-gray-650': props.selected
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
							'border-4 border-gray-500 rounded-sm shadow-md shadow-gray-650 max-h-full max-w-full overflow-hidden'
						)}
						data={props.data}
						size={100}
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
					{props.data?.name}
				</span>
			</div>
		</div>
	);
}
