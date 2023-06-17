import clsx from 'clsx';
import { HTMLAttributes, useEffect, useRef, useState } from 'react';
import { useKey } from 'rooks';
import { FilePath, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { showAlertDialog } from '~/components';
import useClickOutside from '~/hooks/useClickOutside';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';
import { useExplorerViewContext } from '../ViewContext';

interface Props extends HTMLAttributes<HTMLDivElement> {
	filePathData: FilePath;
	activeClassName?: string;
	disabled?: boolean;
}

export default ({ filePathData, className, activeClassName, disabled, ...props }: Props) => {
	const explorerView = useExplorerViewContext();
	const os = useOperatingSystem();

	const ref = useRef<HTMLDivElement>(null);

	const [allowRename, setAllowRename] = useState(false);
	const [renamable, setRenamable] = useState(false);

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
	async function rename() {
		if (!ref.current) return;

		const newName = ref.current.innerText.trim();
		if (!newName) return reset();

		if (!filePathData) return;

		const oldName =
			filePathData.is_dir || !filePathData.extension
				? filePathData.name
				: filePathData.name + '.' + filePathData.extension;

		if (!oldName || !filePathData.location_id || newName === oldName) return;

		try {
			await renameFile.mutateAsync({
				location_id: filePathData.location_id,
				kind: {
					One: {
						from_file_path_id: filePathData.id,
						to: newName
					}
				}
			});
		} catch (e) {
			showAlertDialog({
				title: 'Error',
				value: String(e)
			});
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
			range.setEnd(node, filePathData?.name?.length || 0);

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
			explorerView.setIsRenaming(true);
			setTimeout(() => {
				if (ref.current) {
					ref.current.focus();
					highlightFileName();
				}
			});
		}
	}, [allowRename]);

	// Handle renaming when triggered from outside
	useEffect(() => {
		if (!disabled) {
			if (explorerView.isRenaming && !allowRename) setAllowRename(true);
			else if (!explorerView.isRenaming && allowRename) setAllowRename(false);
		}
	}, [explorerView.isRenaming]);

	useEffect(() => {
		function handleClickOutside(event: MouseEvent) {
			if (ref.current && !ref.current.contains(event.target as Node)) {
				blur();
			}
		}

		document.addEventListener('mousedown', handleClickOutside, true);
		return () => {
			document.removeEventListener('mousedown', handleClickOutside, true);
		};
	}, [ref]);

	// Rename or blur on Enter key
	useKey('Enter', (e) => {
		if (allowRename) {
			e.preventDefault();
			blur();
		} else if (!disabled) setAllowRename(true);
	});

	return (
		<div
			ref={ref}
			role="textbox"
			contentEditable={allowRename}
			suppressContentEditableWarning
			className={clsx(
				'cursor-default overflow-y-auto truncate rounded-md px-1.5 py-px text-xs text-ink',
				allowRename && [
					'whitespace-normal bg-app outline-none ring-2 ring-accent-deep',
					activeClassName
				],
				className
			)}
			onDoubleClick={(e) => e.stopPropagation()}
			onMouseDown={(e) => e.button === 0 && setRenamable(!disabled)}
			onMouseUp={(e) => {
				if (e.button === 0) {
					if (renamable) {
						setAllowRename(true);
					}
					setRenamable(false);
				}
			}}
			onBlur={async () => {
				await rename();
				setAllowRename(false);
				explorerView.setIsRenaming(false);
			}}
			onKeyDown={handleKeyDown}
			{...props}
		>
			{fileName}
		</div>
	);
};
