import clsx from 'clsx';
import { HTMLAttributes, useEffect, useRef, useState } from 'react';
import { useKey } from 'rooks';
import { ExplorerItem, formatBytes, useLibraryMutation } from '@sd/client';
import { tw } from '@sd/ui';
import { getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';
import { getItemFilePath, getItemObject } from '../util';
import ContextMenu from './ContextMenu';
import FileThumb from './Thumb';

interface Props extends HTMLAttributes<HTMLDivElement> {
	data: ExplorerItem;
	selected: boolean;
	index: number;
}

const ItemMetaContainer = tw.div`flex flex-col justify-center`;

function FileItem({ data, selected, index, ...rest }: Props) {
	const objectData = data ? getItemObject(data) : null;
	const filePathData = data ? getItemFilePath(data) : null;
	const os = useOperatingSystem();

	const explorerStore = useExplorerStore();

	const itemNameRef = useRef<HTMLDivElement>(null);

	const [allowRename, setAllowRename] = useState(false);

	const renameFile = useLibraryMutation(['files.renameFile'], {
		onError: () => reset()
	});

	const fileName = `${filePathData?.name}${
		filePathData?.extension && `.${filePathData.extension}`
	}`;

	const reset = () => {
		if (itemNameRef.current) {
			itemNameRef.current.innerText = fileName;
		}
	};

	const rename = () => {
		if (itemNameRef.current) {
			const innerText = itemNameRef.current.innerText.trim();
			if (!innerText) {
				reset();
				return;
			}

			const newName = innerText;
			if (filePathData) {
				const oldName = filePathData.is_dir
					? filePathData.name
					: filePathData.name + '.' + filePathData.extension;

				if (newName !== oldName) {
					renameFile.mutate({
						location_id: filePathData.location_id,
						file_name: oldName,
						new_file_name: newName
					});
				}
			}
		}
	};

	const highlightFileName = () => {
		if (itemNameRef.current) {
			const range = document.createRange();
			const node = itemNameRef.current.firstChild;
			if (!node) return;

			range.setStart(node, 0);
			range.setEnd(node, filePathData?.name.length || 0);

			const sel = window.getSelection();
			sel?.removeAllRanges();
			sel?.addRange(range);
		}
	};

	const blur = () => {
		if (itemNameRef.current) {
			itemNameRef.current.blur();
			setAllowRename(false);
		}
	};

	const handleKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
		switch (e.key) {
			case 'Tab':
				e.preventDefault();
				blur();
				break;

			case 'Escape':
				reset();
				blur();
				break;

			case 'z':
				if (os === 'macOS' ? e.metaKey : e.ctrlKey) {
					reset();
					highlightFileName();
				}
				break;
		}
	};

	useEffect(() => {
		if (allowRename) {
			getExplorerStore().isRenaming = true;
			setTimeout(() => {
				if (itemNameRef.current) {
					itemNameRef.current.focus();
					highlightFileName();
				}
			});
		} else getExplorerStore().isRenaming = false;
	}, [allowRename]);

	useKey('Enter', (e) => {
		if (allowRename) {
			e.preventDefault();
			blur();
		} else if (selected) setAllowRename(true);
	});

	return (
		// Prevent context menu deselecting current item
		<div onClick={(e) => e.stopPropagation()}>
			<ContextMenu data={data} onRename={() => setAllowRename(true)}>
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
						<div
							ref={itemNameRef}
							role="textbox"
							contentEditable={allowRename}
							suppressContentEditableWarning
							className={clsx(
								'cursor-default overflow-y-auto rounded-md px-1.5 py-px text-center text-xs font-medium',
								selected && !allowRename && 'bg-accent text-white',
								allowRename ? 'bg-app' : 'truncate'
							)}
							style={{
								maxHeight: explorerStore.gridItemSize / 3
							}}
							onClick={(e) => {
								if (selected || allowRename) e.stopPropagation();
								if (selected) setAllowRename(true);
							}}
							onBlur={() => {
								rename();
								setAllowRename(false);
							}}
							onKeyDown={handleKeyDown}
						>
							{fileName}
						</div>

						{explorerStore.showBytesInGridView && !allowRename && (
							<span className="text-tiny text-ink-dull cursor-default truncate rounded-md px-1.5 py-px text-center">
								{formatBytes(Number(filePathData?.size_in_bytes || 0))}
							</span>
						)}
					</ItemMetaContainer>
				</div>
			</ContextMenu>
		</div>
	);
}

export default FileItem;
