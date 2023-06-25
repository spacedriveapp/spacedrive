import clsx from 'clsx';
import {
	ComponentProps,
	forwardRef,
	useCallback,
	useEffect,
	useImperativeHandle,
	useRef,
	useState
} from 'react';
import { useKey } from 'rooks';
import { useLibraryMutation, useRspcLibraryContext } from '@sd/client';
import { Tooltip } from '~/../packages/ui/src';
import { showAlertDialog } from '~/components';
import { useIsTextTruncated, useOperatingSystem } from '~/hooks';
import { useExplorerViewContext } from '../ViewContext';

type Props = ComponentProps<'div'> & {
	itemId: number;
	locationId: number | null;
	text: string | null;
	extension?: string | null;
	activeClassName?: string;
	disabled?: boolean;
	renameHandler: (name: string) => Promise<void>;
};

export const RenameTextBoxBase = forwardRef<HTMLDivElement | null, Props>(
	(
		{
			className,
			extension,
			activeClassName,
			disabled,
			itemId,
			locationId,
			text,
			renameHandler,
			...props
		},
		_ref
	) => {
		const explorerView = useExplorerViewContext();
		const os = useOperatingSystem();
		const textRef = useRef<HTMLParagraphElement>(null);
		const [allowRename, setAllowRename] = useState(false);
		const [renamable, setRenamable] = useState(false);

		const ref = useRef<HTMLDivElement>(null);
		useImperativeHandle<HTMLDivElement | null, HTMLDivElement | null>(_ref, () => ref.current);

		// Highlight file name up to extension or
		// fully if it's a directory or has no extension
		const highlightText = useCallback(() => {
			if (ref?.current) {
				const range = document.createRange();
				const node = ref.current.firstChild;
				if (!node) return;

				const endRange = text?.lastIndexOf('.');

				range.setStart(node, 0);
				range.setEnd(node, endRange && endRange !== -1 ? endRange : text?.length || 0);

				const sel = window.getSelection();
				sel?.removeAllRanges();
				sel?.addRange(range);
			}
		}, [text]);

		// Blur field
		function blur() {
			if (ref?.current) {
				ref.current.blur();
				setAllowRename(false);
			}
		}

		// Reset to original file name
		function reset() {
			if (ref?.current) {
				ref.current.innerText = text || '';
			}
		}

		async function handleRename() {
			if (!ref?.current) return;

			const newName = ref?.current.innerText.trim();
			if (!newName) return reset();

			if (!locationId) return;

			const oldName = text;

			if (!oldName || !locationId || newName === oldName) return;

			await renameHandler(newName);
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
						highlightText();
					}
					break;
			}
		}

		//this is to determine if file name is truncated
		const isTruncated = useIsTextTruncated(textRef, text);

		// Focus and highlight when renaming is allowed
		useEffect(() => {
			if (allowRename) {
				explorerView.setIsRenaming(true);
				setTimeout(() => {
					if (ref?.current) {
						ref.current.focus();
						highlightText();
					}
				});
			}
		}, [allowRename, explorerView, highlightText]);

		// Handle renaming when triggered from outside
		useEffect(() => {
			if (!disabled) {
				if (explorerView.isRenaming && !allowRename) setAllowRename(true);
				else if (!explorerView.isRenaming && allowRename) setAllowRename(false);
			}
		}, [explorerView.isRenaming, disabled, allowRename]);

		useEffect(() => {
			function handleClickOutside(event: MouseEvent) {
				if (ref?.current && !ref.current.contains(event.target as Node)) {
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
					await handleRename();
					setAllowRename(false);
					explorerView.setIsRenaming(false);
				}}
				onKeyDown={handleKeyDown}
				{...props}
			>
				{text && (
					<div ref={textRef}>
						{isTruncated ? (
							<Tooltip label={text}>
								<p className="truncate">{text}</p>
							</Tooltip>
						) : (
							text
						)}
					</div>
				)}
			</div>
		);
	}
);

export const RenamePathTextBox = ({
	isDir,
	...props
}: Omit<Props, 'renameHandler'> & { isDir: boolean; extension?: string | null }) => {
	const rspc = useRspcLibraryContext();
	const ref = useRef<HTMLDivElement>(null);

	const renameFile = useLibraryMutation(['files.renameFile'], {
		onError: () => reset(),
		onSuccess: () => rspc.queryClient.invalidateQueries(['search.paths'])
	});

	// Reset to original file name
	function reset() {
		if (ref?.current) {
			ref.current.innerText = props.text || '';
		}
	}

	const fileName = isDir || !props.extension ? props.text : props.text + '.' + props.extension;

	// Handle renaming
	async function rename(newName: string) {
		if (!props.locationId || newName === fileName) return;
		try {
			await renameFile.mutateAsync({
				location_id: props.locationId,
				kind: {
					One: {
						from_file_path_id: props.itemId,
						to: newName
					}
				}
			});
		} catch (e) {
			showAlertDialog({
				title: 'Error',
				value: `Could not rename ${fileName} to ${newName}, due to an error: ${e}`
			});
		}
	}

	return <RenameTextBoxBase {...props} text={fileName} renameHandler={rename} ref={ref} />;
};

export const RenameLocationTextBox = (props: Omit<Props, 'renameHandler'>) => {
	const rspc = useRspcLibraryContext();
	const ref = useRef<HTMLDivElement>(null);

	const renameLocation = useLibraryMutation(['locations.update'], {
		onError: () => reset(),
		onSuccess: () => rspc.queryClient.invalidateQueries(['search.paths'])
	});

	// Reset to original file name
	function reset() {
		if (ref?.current) {
			ref.current.innerText = props.text || '';
		}
	}

	// Handle renaming
	async function rename(newName: string) {
		if (!props.locationId) return;
		try {
			await renameLocation.mutateAsync({
				id: props.locationId,
				name: newName,
				generate_preview_media: null,
				sync_preview_media: null,
				hidden: null,
				indexer_rules_ids: []
			});
		} catch (e) {
			showAlertDialog({
				title: 'Error',
				value: String(e)
			});
		}
	}

	return <RenameTextBoxBase {...props} renameHandler={rename} ref={ref} />;
};
