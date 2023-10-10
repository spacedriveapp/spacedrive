import clsx from 'clsx';
import { forwardRef, useCallback, useEffect, useImperativeHandle, useRef, useState } from 'react';
import { useKey } from 'rooks';
import { Tooltip } from '@sd/ui';
import { useIsTextTruncated, useOperatingSystem } from '~/hooks';

import { useExplorerViewContext } from '../ViewContext';

interface Props extends React.HTMLAttributes<HTMLDivElement> {
	name: string;
	onRename: (newName: string) => void;
	disabled?: boolean;
}

export const RenameTextBox = forwardRef<HTMLDivElement, Props>(
	({ name, onRename, disabled, className, ...props }, _ref) => {
		const explorerView = useExplorerViewContext();
		const os = useOperatingSystem();

		const [allowRename, setAllowRename] = useState(false);

		const renamable = useRef<boolean>(false);
		const timeout = useRef<NodeJS.Timeout | null>(null);

		const ref = useRef<HTMLDivElement>(null);
		useImperativeHandle<HTMLDivElement | null, HTMLDivElement | null>(_ref, () => ref.current);

		//this is to determine if file name is truncated
		const isTruncated = useIsTextTruncated(ref, name);

		// Highlight file name up to extension or
		// fully if it's a directory, hidden file or has no extension
		const highlightText = useCallback(() => {
			if (!ref.current || !name) return;

			const node = ref.current.firstChild;
			if (!node) return;

			const endRange = name.lastIndexOf('.');

			const range = document.createRange();

			range.setStart(node, 0);
			range.setEnd(node, endRange > 1 ? endRange : name.length);

			const sel = window.getSelection();
			if (!sel) return;

			sel.removeAllRanges();
			sel.addRange(range);
		}, [name]);

		// Blur field
		const blur = useCallback(() => ref.current?.blur(), []);

		// Reset to original file name
		const reset = () => ref.current && (ref.current.innerText = name ?? '');

		const handleRename = async () => {
			let newName = ref.current?.innerText;

			if (newName?.endsWith('\n')) newName = newName.slice(0, -1);

			if (!newName || newName === name) {
				reset();
				return;
			}

			onRename(newName);
		};

		const handleKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
			switch (e.key) {
				case 'Tab': {
					e.preventDefault();
					blur();
					break;
				}

				case 'Escape': {
					e.stopPropagation();
					reset();
					blur();
					break;
				}

				case 'z': {
					if (os === 'macOS' ? e.metaKey : e.ctrlKey) {
						reset();
						highlightText();
					}
				}
			}
		};

		const resetState = () => {
			setAllowRename(false);
			renamable.current = false;
			if (timeout.current) {
				clearTimeout(timeout.current);
				timeout.current = null;
			}
		};

		useKey(['F2', 'Enter'], (e) => {
			e.preventDefault();
			if (os === 'windows' && e.key === 'Enter') return;
			if (allowRename) blur();
			else if (!disabled) setAllowRename(true);
		});

		useEffect(() => {
			const element = ref.current;
			if (!element || !allowRename) return;

			const scroll = (e: WheelEvent) => {
				e.preventDefault();
				element.scrollTop += e.deltaY;
			};

			highlightText();

			element.addEventListener('wheel', scroll);
			return () => element.removeEventListener('wheel', scroll);
		}, [allowRename, highlightText]);

		useEffect(() => {
			if (!disabled) {
				if (explorerView.isRenaming && !allowRename) setAllowRename(true);
				else explorerView.setIsRenaming(allowRename);
			} else resetState();
		}, [explorerView.isRenaming, disabled, allowRename, explorerView]);

		useEffect(() => {
			const onMouseDown = (event: MouseEvent) => {
				if (!ref.current?.contains(event.target as Node)) blur();
			};

			document.addEventListener('mousedown', onMouseDown, true);
			return () => document.removeEventListener('mousedown', onMouseDown, true);
		}, [blur]);

		return (
			<Tooltip
				labelClassName="break-all"
				tooltipClassName="!max-w-[250px]"
				label={!isTruncated || allowRename ? null : name}
				asChild
			>
				<div
					ref={ref}
					role="textbox"
					autoCorrect="off"
					contentEditable={allowRename}
					suppressContentEditableWarning
					className={clsx(
						'cursor-default truncate rounded-md px-1.5 py-px text-xs text-ink outline-none',
						allowRename && 'whitespace-normal bg-app ring-2 ring-accent-deep',
						className
					)}
					onDoubleClick={(e) => {
						if (allowRename) e.stopPropagation();
						renamable.current = false;
					}}
					onMouseDownCapture={(e) => e.button === 0 && (renamable.current = !disabled)}
					onMouseUp={(e) => {
						if (e.button === 0 || renamable.current || !allowRename) {
							timeout.current = setTimeout(
								() => renamable.current && setAllowRename(true),
								350
							);
						}
					}}
					onBlur={() => {
						handleRename();
						resetState();
						explorerView.setIsRenaming(false);
					}}
					onKeyDown={handleKeyDown}
					{...props}
				>
					{name}
				</div>
			</Tooltip>
		);
	}
);
