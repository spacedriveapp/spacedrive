import clsx from 'clsx';
import { HTMLAttributes, useEffect, useRef, useState } from 'react';
import { useKey } from 'rooks';
import { FilePath, useLibraryMutation } from '@sd/client';
import { getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';

interface Props extends HTMLAttributes<HTMLDivElement> {
	filePathData: FilePath;
	selected: boolean;
	activeClassName?: string;
}

export default ({ filePathData, selected, className, activeClassName, ...props }: Props) => {
	const explorerStore = useExplorerStore();
	const os = useOperatingSystem();

	const ref = useRef<HTMLDivElement>(null);

	const [allowRename, setAllowRename] = useState(false);

	const renameFile = useLibraryMutation(['files.renameFile'], {
		onError: () => reset()
	});

	const fileName = `${filePathData?.name}${
		filePathData?.extension && `.${filePathData.extension}`
	}`;

	// Reset to original file name
	function reset() {
		if (ref.current) {
			ref.current.innerText = fileName;
		}
	}

	// Handle renaming
	function rename() {
		if (ref.current) {
			const innerText = ref.current.innerText.trim();
			if (!innerText) return reset();

			const newName = innerText;
			if (filePathData) {
				const oldName =
					filePathData.is_dir || !filePathData.extension
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
	}

	// Highlight file name up to extension or
	// fully if it's a directory or has no extension
	function highlightFileName() {
		if (ref.current) {
			const range = document.createRange();
			const node = ref.current.firstChild;
			if (!node) return;

			range.setStart(node, 0);
			range.setEnd(node, filePathData?.name.length || 0);

			const sel = window.getSelection();
			sel?.removeAllRanges();
			sel?.addRange(range);
		}
	}

	// Blur field
	function blur() {
		if (ref.current) {
			ref.current.blur();
			setAllowRename(false);
		}
	}

	// Handle keydown events
	function handleKeyDown(e: React.KeyboardEvent<HTMLDivElement>) {
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
	}

	// Focus and highlight when renaming is allowed
	useEffect(() => {
		if (allowRename) {
			getExplorerStore().isRenaming = true;
			setTimeout(() => {
				if (ref.current) {
					ref.current.focus();
					highlightFileName();
				}
			});
		} else getExplorerStore().isRenaming = false;
	}, [allowRename]);

	// Handle renaming when triggered from outside
	useEffect(() => {
		if (selected) {
			if (explorerStore.isRenaming && !allowRename) setAllowRename(true);
			else if (!explorerStore.isRenaming && allowRename) setAllowRename(false);
		}
	}, [explorerStore.isRenaming]);

	// Rename or blur on Enter key
	useKey('Enter', (e) => {
		if (allowRename) {
			e.preventDefault();
			blur();
		} else if (selected) setAllowRename(true);
	});

	return (
		<div
			ref={ref}
			role="textbox"
			contentEditable={allowRename}
			suppressContentEditableWarning
			className={clsx(
				'cursor-default overflow-y-auto truncate rounded-md px-1.5 py-px text-xs',
				allowRename && ['whitespace-normal bg-app', activeClassName],
				className
			)}
			onClick={(e) => {
				if (selected || allowRename) e.stopPropagation();
				if (selected) setAllowRename(true);
			}}
			onBlur={() => {
				rename();
				setAllowRename(false);
			}}
			onKeyDown={handleKeyDown}
			{...props}
		>
			{fileName}
		</div>
	);
};
