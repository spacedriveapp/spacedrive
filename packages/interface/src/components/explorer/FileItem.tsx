import clsx from 'clsx';
import { HTMLAttributes } from 'react';
import { ExplorerItem, isVideoExt } from '@sd/client';
import { cva, tw } from '@sd/ui';
import { getExplorerStore } from '~/hooks/useExplorerStore';
import { ObjectKind } from '~/util/kind';
import { FileItemContextMenu } from './ExplorerContextMenu';
import FileThumb from './FileThumb';
import { isObject } from './utils';

const NameArea = tw.div`flex justify-center`;

const nameContainerStyles = cva(
	'px-1.5 py-[1px] truncate text-center rounded-md text-xs font-medium cursor-default',
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
	const isVid = isVideoExt(data.item.extension || '');
	const item = data.item;

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
				className={clsx('inline-block w-[100px] mb-3', rest.className)}
			>
				<div
					style={{
						width: getExplorerStore().gridItemSize,
						height: getExplorerStore().gridItemSize
					}}
					className={clsx(
						'border-2 border-transparent rounded-lg text-center mb-1 active:translate-y-[1px]',
						{
							'bg-app-selected/30': selected
						}
					)}
				>
					<div
						className={clsx(
							'flex relative items-center justify-center h-full p-1 rounded border-transparent border-2 shrink-0'
						)}
					>
						<FileThumb
							className={clsx(
								'border-2 border-app-line rounded-sm shadow shadow-black/40 object-cover max-w-full max-h-full w-auto overflow-hidden',
								isVid && '!border-black rounded border-x-0 border-y-[7px]'
							)}
							data={data}
							kind={ObjectKind[objectData?.kind || 0]}
							size={getExplorerStore().gridItemSize}
						/>
						{item.extension && isVid && (
							<div className="absolute bottom-4 font-semibold opacity-70 right-2 py-0.5 px-1 text-[9px] uppercase bg-black/60 rounded">
								{item.extension}
							</div>
						)}
					</div>
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
