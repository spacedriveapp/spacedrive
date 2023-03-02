import clsx from 'clsx';
import { HTMLAttributes } from 'react';
import { ExplorerItem, ObjectKind, isObject } from '@sd/client';
import { getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import ContextMenu from './ContextMenu';
import FileThumb from './Thumb';

interface Props extends HTMLAttributes<HTMLDivElement> {
	data: ExplorerItem;
	selected: boolean;
	index: number;
}

function FileItem({ data, selected, index, ...rest }: Props) {
	const item = data.item;

	const explorerStore = useExplorerStore();

	return (
		<ContextMenu data={data}>
			<div
				onContextMenu={() => {
					if (index != undefined) {
						getExplorerStore().selectedRowIndex = index;
					}
				}}
				{...rest}
				draggable
				style={{ width: explorerStore.gridItemSize }}
				className={clsx('mb-3 inline-block', rest.className)}
			>
				<div
					style={{
						width: explorerStore.gridItemSize,
						height: explorerStore.gridItemSize
					}}
					className={clsx(
						'mb-1 rounded-lg border-2 border-transparent text-center active:translate-y-[1px]',
						{
							'bg-app-selected/20': selected
						}
					)}
				>
					<FileThumb data={data} size={explorerStore.gridItemSize} />
				</div>
				<div className="flex justify-center">
					<span
						className={clsx(
							'cursor-default truncate rounded-md px-1.5 py-[1px] text-center text-xs font-medium',
							selected && 'bg-accent text-white'
						)}
					>
						{item.name}
						{item.extension && `.${item.extension}`}
					</span>
				</div>
			</div>
		</ContextMenu>
	);
}

export default FileItem;
