import clsx from 'clsx';
import {
	forwardRef,
	useCallback,
	useEffect,
	useImperativeHandle,
	useRef,
	useState,
	type ComponentProps
} from 'react';
import { useKey } from 'rooks';
import { useLibraryMutation, useRspcLibraryContext } from '@sd/client';
import { toast, Tooltip } from '@sd/ui';
import { useIsTextTruncated, useOperatingSystem } from '~/hooks';

import { useExplorerViewContext } from '../ViewContext';

type Props = ComponentProps<'div'> & {
	itemId?: null | number;
	locationId: number | null;
	text: string | null;
	activeClassName?: string;
	disabled?: boolean;
	renameHandler: (name: string) => Promise<void>;
};

export const RenameTextBoxBase = forwardRef<HTMLDivElement | null, Props>(
	(
		{ className, activeClassName, disabled, itemId, locationId, text, renameHandler, ...props },
		_ref
	) => {
		const explorerView = useExplorerViewContext();
		const os = useOperatingSystem();

		const [allowRename, setAllowRename] = useState(false);

		const renamable = useRef<boolean>(false);
		const timeout = useRef<NodeJS.Timeout | null>(null);

		const ref = useRef<HTMLDivElement>(null);
		useImperativeHandle<HTMLDivElement | null, HTMLDivElement | null>(_ref, () => ref.current);

		//this is to determine if file name is truncated
		const isTruncated = useIsTextTruncated(ref, text);

		// Highlight file name up to extension or
		// fully if it's a directory or has no extension
		const highlightText = useCallback(() => {
			if (!ref.current || !text) return;

			const node = ref.current.firstChild;
			if (!node) return;

			const endRange = text.lastIndexOf('.');

			const range = document.createRange();

			range.setStart(node, 0);
			range.setEnd(node, endRange !== -1 ? endRange : text.length);

			const sel = window.getSelection();
			if (!sel) return;

			sel.removeAllRanges();
			sel.addRange(range);
		}, [text]);

		// Blur field
		const blur = useCallback(() => ref.current?.blur(), []);

		// Reset to original file name
		const reset = () => ref.current && (ref.current.innerText = text ?? '');

		const handleRename = async () => {
			const newName = ref.current?.innerText.trim();

			if (!newName || newName === text) {
				reset();
				return;
			}

			await renameHandler(newName);
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

		useKey('Enter', (e) => {
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
			<Tooltip label={!isTruncated || allowRename ? null : text} asChild>
				<div
					ref={ref}
					role="textbox"
					contentEditable={allowRename}
					suppressContentEditableWarning
					className={clsx(
						'cursor-default truncate rounded-md px-1.5 py-px text-xs text-ink outline-none',
						allowRename && [
							'whitespace-normal bg-app ring-2 ring-accent-deep',
							activeClassName
						],
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
					{text}
				</div>
			</Tooltip>
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
			ref.current.innerText = fileName ?? '';
		}
	}

	const fileName = isDir || !props.extension ? props.text : props.text + '.' + props.extension;

	// Handle renaming
	async function rename(newName: string) {
		if (!props.locationId || !props.itemId || newName === fileName) {
			reset();
			return;
		}
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
			reset();
			toast.error({
				title: `Could not rename ${fileName} to ${newName}`,
				body: `Error: ${e}.`
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
		if (!props.locationId) {
			reset();
			return;
		}
		try {
			await renameLocation.mutateAsync({
				id: props.locationId,
				path: null,
				name: newName,
				generate_preview_media: null,
				sync_preview_media: null,
				hidden: null,
				indexer_rules_ids: []
			});
		} catch (e) {
			reset();
			toast.error({ title: 'Failed to rename', body: `Error: ${e}.` });
		}
	}

	return <RenameTextBoxBase {...props} renameHandler={rename} ref={ref} />;
};
