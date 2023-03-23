import clsx from 'clsx';
import { HTMLAttributes } from 'react';
import { ExplorerItem, ObjectKind, formatBytes, isObject, isPath } from '@sd/client';
import { tw } from '@sd/ui';
import { getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import ContextMenu from './ContextMenu';
import FileThumb from './Thumb';

interface Props extends HTMLAttributes<HTMLDivElement> {
	data: ExplorerItem;
	selected: boolean;
	index: number;
}

const ItemMetaContainer = tw.div`flex flex-col justify-center`;

function FileItem({ data, selected, index, ...rest }: Props) {
	const objectData = data ? (isObject(data) ? data.item : data.item.object) : null;

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
				<ItemMetaContainer>
					<span
						className={clsx(
							'cursor-default truncate rounded-md px-1.5 py-[1px] text-center text-xs font-medium',
							selected && 'bg-accent text-white'
						)}
					>
						{data.item.name}
						{data.item.extension && `.${data.item.extension}`}
					</span>
					{explorerStore.showBytesInGridView && (
						<span
							className={clsx(
								'text-tiny text-ink-dull cursor-default truncate rounded-md px-1.5 py-[1px] text-center '
							)}
						>
							{formatBytes(Number(objectData?.size_in_bytes || 0))}
						</span>
					)}
				</ItemMetaContainer>
			</div>
		</ContextMenu>
	);
}

export default FileItem;
