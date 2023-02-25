import clsx from 'clsx';
import { HTMLAttributes } from 'react';
import { ExplorerItem, ObjectKind, isObject } from '@sd/client';
import { cva, tw } from '@sd/ui';
import { getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import { FileItemContextMenu } from './ExplorerContextMenu';
import { FileThumb } from './FileThumb';

const NameArea = tw.div`flex justify-center`;

const nameContainerStyles = cva(
	'cursor-default truncate rounded-md px-1.5 py-[1px] text-center text-xs font-medium',
	{
		variants: {
			selected: {
				true: 'bg-accent text-white'
			}
		}
	}
);

interface Props extends HTMLAttributes<HTMLDivElement> {
	data: ExplorerItem;
	selected: boolean;
	index: number;
}

function FileItem({ data, selected, index, ...rest }: Props) {
	const objectData = data ? (isObject(data) ? data.item : data.item.object) : null;
	const isVid = ObjectKind[objectData?.kind || 0] === 'Video';
	const item = data.item;

	const explorerStore = useExplorerStore();

	return (
		<FileItemContextMenu data={data}>
			<div
				onContextMenu={(e) => {
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
				<NameArea>
					<span className={nameContainerStyles({ selected })}>
						{item.name}
						{item.extension && `.${item.extension}`}
					</span>
				</NameArea>
			</div>
		</FileItemContextMenu>
	);
}

export default FileItem;
