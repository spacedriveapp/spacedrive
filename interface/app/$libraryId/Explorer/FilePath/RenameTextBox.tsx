import clsx from 'clsx';
import {
	forwardRef,
	memo,
	ReactElement,
	ReactNode,
	useCallback,
	useEffect,
	useImperativeHandle,
	useRef,
	useState
} from 'react';
import TruncateMarkup from 'react-truncate-markup';
import { useSelector } from '@sd/client';
import { dialogManager, Tooltip } from '@sd/ui';
import { useOperatingSystem, useShortcut } from '~/hooks';

import { explorerStore } from '../store';

export interface RenameTextBoxProps extends React.HTMLAttributes<HTMLDivElement> {
	name: string;
	onRename: (newName: string) => void;
	disabled?: boolean;
	/**
	 * Number of text lines to display in idle.
	 *
	 * @defaultValue `1`
	 */
	lines?: number;
	/**
	 * Number of text lines to display when renaming.
	 */
	editLines?: number;
	/**
	 * Determines how the rename text box is toggled.
	 *
	 * - `shortcut`: Toggled by the `renameObject` shortcut and `explorerStore.isRenaming` value.
	 * - `click`: Toggled by clicking on the text box.
	 * - `all`: Toggled by both shortcut and click.
	 *
	 * @defaultValue `all`
	 */
	toggleBy?: 'shortcut' | 'click' | 'all';
	idleClassName?: string;
	activeClassName?: string;
}

export const RenameTextBox = forwardRef<HTMLDivElement, RenameTextBoxProps>(
	(
		{
			name,
			onRename,
			disabled,
			lines = 1,
			editLines,
			toggleBy = 'all',
			className,
			idleClassName,
			activeClassName,
			style,
			...props
		},
		_ref
	) => {
		const os = useOperatingSystem();

		const [isRenaming, drag] = useSelector(explorerStore, (s) => [s.isRenaming, s.drag]);

		const ref = useRef<HTMLDivElement>(null);
		useImperativeHandle<HTMLDivElement | null, HTMLDivElement | null>(_ref, () => ref.current);

		const truncateMarkup = useRef<TruncateMarkup>(null);

		const renamable = useRef<boolean>(false);
		const timeout = useRef<number | null>(null);

		const [allowRename, setAllowRename] = useState(false);
		const [isTruncated, setIsTruncated] = useState(false);

		// Height of a single line of text
		// Used to set the max height of the text box
		const [lineHeight, setLineHeight] = useState(0);

		// Padding of the text box
		// Included in the max height calculation
		const [paddingTop, setPaddingTop] = useState(0);
		const [paddingBottom, setPaddingBottom] = useState(0);

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
		const reset = () => ref.current && (ref.current.innerText = name);

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
			e.stopPropagation();

			switch (e.key) {
				case 'Tab':
				case 'Enter': {
					e.preventDefault();
					blur();
					break;
				}
				case 'Escape': {
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

		const toggleRename = useCallback(() => {
			setAllowRename(true);

			const textBoxNode = ref.current;
			if (!textBoxNode) return;

			const { paddingTop, paddingBottom } = getComputedStyle(textBoxNode);

			setPaddingTop(parseFloat(paddingTop));
			setPaddingBottom(parseFloat(paddingBottom));

			const markup = truncateMarkup.current;
			if (!markup) return;

			// @ts-ignore
			// Passing ref to TruncateMarkup child doesn't work, so we have
			// to access the element directly from the markup instance
			const textNode = markup.el as HTMLElement;

			const { lineHeight } = getComputedStyle(textNode);

			const textLines =
				lines !== 1 ? Math.round(textNode.clientHeight / parseFloat(lineHeight)) : lines;

			setLineHeight(textNode.clientHeight / textLines);
		}, [lines]);

		useShortcut('renameObject', (e) => {
			if (dialogManager.isAnyDialogOpen() || toggleBy === 'click') return;
			e.preventDefault();
			if (allowRename) blur();
			else if (!disabled) toggleRename();
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
			if (toggleBy === 'click') return;
			if (!disabled) {
				if (isRenaming && !allowRename) toggleRename();
				else explorerStore.isRenaming = allowRename;
			} else resetState();
		}, [isRenaming, disabled, allowRename, toggleBy, toggleRename]);

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
						allowRename
							? [
									'whitespace-normal bg-app !text-ink ring-2 ring-accent-deep',
									activeClassName
								]
							: [idleClassName],

						className
					)}
					style={{
						maxHeight:
							!allowRename && lines === 1
								? '1lh' // limit height to 1 line as TruncateMarkup likes to wrap text on resize - needs to be fixed
								: allowRename && lineHeight && editLines
									? editLines * lineHeight + paddingTop + paddingBottom
									: undefined,
						...style
					}}
					onDoubleClick={(e) => {
						if (allowRename) e.stopPropagation();
						renamable.current = false;
					}}
					onMouseDownCapture={(e) => {
						if (allowRename) e.stopPropagation();
						if (e.button === 0) renamable.current = !disabled;
					}}
					onMouseUp={(e) => {
						if (e.button === 0 || renamable.current || !allowRename) {
							timeout.current = setTimeout(
								() => renamable.current && toggleRename(),
								350
							);
						}
					}}
					onBlur={() => {
						explorerStore.isRenaming = false;
						handleRename();
						resetState();
					}}
					onKeyDown={handleKeyDown}
					{...props}
				>
					{allowRename ? (
						name
					) : (
						<TruncatedText
							ref={truncateMarkup}
							text={name}
							lines={lines}
							onTruncate={setIsTruncated}
						/>
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

const TruncatedText = memo(
	forwardRef<TruncateMarkup, TruncatedTextProps>(({ text, lines = 1, onTruncate }, ref) => {
		const ellipsis = useCallback(
			(rootEl: ReactNode) => {
				const truncatedText = (rootEl as ReactElement<{ children: string }>).props.children;
				return `...${text.slice(-(truncatedText.length / lines))}`;
			},
			[text, lines]
		);

		return (
			<TruncateMarkup ref={ref} lines={lines} ellipsis={ellipsis} onTruncate={onTruncate}>
				<div>{text}</div>
			</TruncateMarkup>
		);
	})
);

TruncatedText.displayName = 'TruncatedText';
