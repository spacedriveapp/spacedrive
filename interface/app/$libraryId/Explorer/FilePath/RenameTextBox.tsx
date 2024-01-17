import clsx from 'clsx';
import {
	forwardRef,
	memo,
	useCallback,
	useEffect,
	useImperativeHandle,
	useRef,
	useState
} from 'react';
import TruncateMarkup from 'react-truncate-markup';
import { useSelector } from '@sd/client';
import { Tooltip } from '@sd/ui';
import { useOperatingSystem, useShortcut } from '~/hooks';

import { explorerStore } from '../store';

export interface RenameTextBoxProps extends React.HTMLAttributes<HTMLDivElement> {
	name: string;
	onRename: (newName: string) => void;
	disabled?: boolean;
	lines?: number;
	// Temporary solution for TruncatedText in list view
	idleClassName?: string;
}

export const RenameTextBox = forwardRef<HTMLDivElement, RenameTextBoxProps>(
	({ name, onRename, disabled, className, idleClassName, lines, ...props }, _ref) => {
		const os = useOperatingSystem();
		const [isRenaming, drag] = useSelector(explorerStore, (s) => [s.isRenaming, s.drag]);

		const ref = useRef<HTMLDivElement>(null);
		useImperativeHandle<HTMLDivElement | null, HTMLDivElement | null>(_ref, () => ref.current);

		const renamable = useRef<boolean>(false);
		const timeout = useRef<number | null>(null);

		const [allowRename, setAllowRename] = useState(false);
		const [isTruncated, setIsTruncated] = useState(false);

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

		useShortcut('renameObject', (e) => {
			e.preventDefault();
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
				if (isRenaming && !allowRename) setAllowRename(true);
				else explorerStore.isRenaming = allowRename;
			} else resetState();
		}, [isRenaming, disabled, allowRename]);

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
				label={!isTruncated || allowRename || drag?.type === 'dragging' ? null : name}
				asChild
			>
				<div
					ref={ref}
					role="textbox"
					autoCorrect="off"
					contentEditable={allowRename}
					suppressContentEditableWarning
					className={clsx(
						'cursor-default overflow-hidden rounded-md px-1.5 py-px text-xs text-ink outline-none',
						allowRename && 'whitespace-normal bg-app !text-ink ring-2 ring-accent-deep',
						!allowRename && idleClassName,
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
						explorerStore.isRenaming = false;
					}}
					onKeyDown={handleKeyDown}
					{...props}
				>
					{allowRename ? (
						name
					) : (
						<TruncatedText text={name} lines={lines} onTruncate={setIsTruncated} />
					)}
				</div>
			</Tooltip>
		);
	}
);

RenameTextBox.displayName = 'RenameTextBox';

interface TruncatedTextProps {
	text: string;
	lines?: number;
	onTruncate: (wasTruncated: boolean) => void;
}

const TruncatedText = memo(({ text, lines, onTruncate }: TruncatedTextProps) => {
	const ellipsis = useCallback(() => {
		const extension = text.lastIndexOf('.');
		if (extension !== -1) return `...${text.slice(-(text.length - extension + 2))}`;
		return `...${text.slice(-8)}`;
	}, [text]);

	return (
		<TruncateMarkup lines={lines} ellipsis={ellipsis} onTruncate={onTruncate}>
			<div>{text}</div>
		</TruncateMarkup>
	);
});

TruncatedText.displayName = 'TruncatedText';
