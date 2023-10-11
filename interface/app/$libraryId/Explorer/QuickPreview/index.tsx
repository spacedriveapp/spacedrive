import { ArrowLeft, ArrowRight, DotsThree, Plus, SidebarSimple, X } from '@phosphor-icons/react';
import * as Dialog from '@radix-ui/react-dialog';
import clsx from 'clsx';
import {
	ButtonHTMLAttributes,
	createContext,
	createRef,
	useCallback,
	useContext,
	useEffect,
	useMemo,
	useRef,
	useState
} from 'react';
import {
	getExplorerItemData,
	getIndexedItemFilePath,
	ObjectKindKey,
	useLibraryContext,
	useLibraryMutation,
	useRspcLibraryContext,
	useZodForm
} from '@sd/client';
import {
	dialogManager,
	DropdownMenu,
	Form,
	ModifierKeys,
	toast,
	ToastMessage,
	Tooltip,
	z
} from '@sd/ui';
import { useIsDark, useKeybind, useOperatingSystem } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { useExplorerContext } from '../Context';
import ExplorerContextMenu, {
	FilePathItems,
	ObjectItems,
	SeparatedConditional,
	SharedItems
} from '../ContextMenu';
import { Conditional } from '../ContextMenu/ConditionalItem';
import DeleteDialog from '../FilePath/DeleteDialog';
import { FileThumb } from '../FilePath/Thumb';
import { SingleItemMetadata } from '../Inspector';
import { getQuickPreviewStore, useQuickPreviewStore } from './store';

const iconKinds: ObjectKindKey[] = ['Audio', 'Folder', 'Executable', 'Unknown'];
const textKinds: ObjectKindKey[] = ['Text', 'Config', 'Code'];
const withoutBackgroundKinds: ObjectKindKey[] = [...iconKinds, ...textKinds, 'Document'];

const QuickPreviewContext = createContext<{ background: boolean } | null>(null);

const useQuickPreviewContext = () => {
	const context = useContext(QuickPreviewContext);

	if (!context) throw new Error('QuickPreviewContext.Provider not found!');

	return context;
};

export const QuickPreview = () => {
	const os = useOperatingSystem();
	const rspc = useRspcLibraryContext();
	const isDark = useIsDark();
	const { library } = useLibraryContext();
	const { openFilePaths, revealItems, openEphemeralFiles } = usePlatform();

	const explorer = useExplorerContext();
	const { open, itemIndex } = useQuickPreviewStore();

	const thumb = createRef<HTMLDivElement>();
	const [thumbErrorToast, setThumbErrorToast] = useState<ToastMessage>();
	const [showMetadata, setShowMetadata] = useState<boolean>(false);
	const [isContextMenuOpen, setIsContextMenuOpen] = useState<boolean>(false);
	const [isRenaming, setIsRenaming] = useState<boolean>(false);
	const [newName, setNewName] = useState<string | null>(null);

	const items = useMemo(
		() => (open ? [...explorer.selectedItems] : []),
		[explorer.selectedItems, open]
	);

	const item = useMemo(() => items[itemIndex], [items, itemIndex]);

	const renameFile = useLibraryMutation(['files.renameFile'], {
		onError: () => setNewName(null),
		onSuccess: () => rspc.queryClient.invalidateQueries(['search.paths'])
	});

	const changeCurrentItem = (index: number) => {
		if (items[index]) getQuickPreviewStore().itemIndex = index;
	};

	// Error toast
	useEffect(() => {
		if (!thumbErrorToast) return;

		let id: string | number | undefined;
		toast.error(
			(_id) => {
				id = _id;
				return thumbErrorToast;
			},
			{
				ref: thumb,
				duration: Infinity,
				onClose() {
					id = undefined;
					setThumbErrorToast(undefined);
				}
			}
		);

		return () => void toast.dismiss(id);
	}, [thumb, thumbErrorToast]);

	// Reset state
	useEffect(() => {
		setNewName(null);
		setThumbErrorToast(undefined);

		if (open || item) return;

		getQuickPreviewStore().open = false;
		getQuickPreviewStore().itemIndex = 0;
		setShowMetadata(false);
	}, [item, open]);

	// Toggle quick preview
	useKeybind(['space'], (e) => {
		if (isRenaming) return;

		e.preventDefault();

		getQuickPreviewStore().open = !open;
	});

	useKeybind('Escape', (e) => open && e.stopPropagation());

	// Move between items
	useKeybind([['left'], ['right']], (e) => {
		if (isContextMenuOpen || isRenaming) return;
		changeCurrentItem(e.key === 'ArrowLeft' ? itemIndex - 1 : itemIndex + 1);
	});

	// Toggle metadata
	useKeybind([os === 'macOS' ? ModifierKeys.Meta : ModifierKeys.Control, 'i'], () =>
		setShowMetadata(!showMetadata)
	);

	// Open file
	useKeybind([os === 'macOS' ? ModifierKeys.Meta : ModifierKeys.Control, 'o'], () => {
		if (!item || !openFilePaths || !openEphemeralFiles) return;

		try {
			if (item.type === 'Path' || item.type === 'Object') {
				const path = getIndexedItemFilePath(item);

				if (!path) throw 'No path found';

				openFilePaths(library.uuid, [path.id]);
			} else if (item.type === 'NonIndexedPath') {
				openEphemeralFiles([item.item.path]);
			}
		} catch (error) {
			toast.error({
				title: 'Failed to open file',
				body: `Couldn't open file, due to an error: ${error}`
			});
		}
	});

	// Reveal in native explorer
	useKeybind([os === 'macOS' ? ModifierKeys.Meta : ModifierKeys.Control, 'y'], () => {
		if (!item || !revealItems) return;

		try {
			const toReveal = [];
			if (item.type === 'Location') {
				toReveal.push({ Location: { id: item.item.id } });
			} else if (item.type === 'NonIndexedPath') {
				toReveal.push({ Ephemeral: { path: item.item.path } });
			} else {
				const filePath = getIndexedItemFilePath(item);
				if (!filePath) throw 'No file path found';
				toReveal.push({ FilePath: { id: filePath.id } });
			}

			revealItems(library.uuid, toReveal);
		} catch (error) {
			toast.error({
				title: 'Failed to reveal',
				body: `Couldn't reveal file, due to an error: ${error}`
			});
		}
	});

	// Open delete dialog
	useKeybind([os === 'macOS' ? ModifierKeys.Meta : ModifierKeys.Control, 'backspace'], () => {
		if (!item) return;

		const path = getIndexedItemFilePath(item);

		if (!path || path.location_id === null) return;

		dialogManager.create((dp) => (
			<DeleteDialog
				{...dp}
				locationId={path.location_id!}
				pathIds={[path.id]}
				includesDirectorys={path.is_dir ?? false}
				includesFiles={!path.is_dir}
			/>
		));
	});

	if (!item) return null;

	const { kind, ...itemData } = getExplorerItemData(item);

	const name = newName || `${itemData.name}${itemData.extension ? `.${itemData.extension}` : ''}`;

	const background = !withoutBackgroundKinds.includes(kind);
	const icon = iconKinds.includes(kind);

	return (
		<Dialog.Root open={open} onOpenChange={(open) => (getQuickPreviewStore().open = open)}>
			<QuickPreviewContext.Provider value={{ background }}>
				<Dialog.Portal forceMount>
					<Dialog.Overlay
						className={clsx(
							'absolute inset-0 z-50',
							'radix-state-open:animate-in radix-state-open:fade-in-0',
							isDark ? 'bg-black/80' : 'bg-black/60'
						)}
						onContextMenu={(e) => e.preventDefault()}
					/>

					<Dialog.Content
						className="fixed inset-[5%] z-50 outline-none radix-state-open:animate-in radix-state-open:fade-in-0 radix-state-open:zoom-in-95"
						onOpenAutoFocus={(e) => e.preventDefault()}
						onEscapeKeyDown={(e) => isRenaming && e.preventDefault()}
						onContextMenu={(e) => e.preventDefault()}
						onInteractOutside={(e) => {
							if (
								e.target &&
								e.target instanceof Node &&
								thumb.current?.contains(e.target)
							)
								e.preventDefault();
						}}
					>
						<div
							className={clsx(
								'flex h-full overflow-hidden rounded-md border',
								isDark ? 'border-app-line/80' : 'border-app-line/10'
							)}
						>
							<div className="relative flex flex-1 flex-col overflow-hidden bg-app/80 backdrop-blur">
								{background && (
									<div className="absolute inset-0 overflow-hidden">
										<FileThumb
											data={item}
											cover={true}
											childClassName="scale-125"
										/>
										<div className="absolute inset-0 bg-black/25 backdrop-blur-3xl" />
									</div>
								)}
								<div
									className={clsx(
										'z-50 flex items-center p-2',
										background ? 'text-white' : 'text-ink'
									)}
								>
									<div className="flex flex-1">
										<Tooltip label="Close">
											<Dialog.Close asChild>
												<IconButton>
													<X weight="bold" />
												</IconButton>
											</Dialog.Close>
										</Tooltip>

										{items.length > 1 && (
											<div className="ml-2 flex">
												<Tooltip label="Back">
													<IconButton
														disabled={!items[itemIndex - 1]}
														onClick={() =>
															changeCurrentItem(itemIndex - 1)
														}
														className="rounded-r-none"
													>
														<ArrowLeft weight="bold" />
													</IconButton>
												</Tooltip>

												<Tooltip label="Forward">
													<IconButton
														disabled={!items[itemIndex + 1]}
														onClick={() =>
															changeCurrentItem(itemIndex + 1)
														}
														className="rounded-l-none"
													>
														<ArrowRight weight="bold" />
													</IconButton>
												</Tooltip>
											</div>
										)}
									</div>

									<div className="flex w-1/2 items-center justify-center truncate text-sm">
										{isRenaming && name ? (
											<RenameInput
												name={name}
												onRename={(newName) => {
													setIsRenaming(false);

													if (
														!('id' in item.item) ||
														!newName ||
														newName === name
													)
														return;

													const filePathData =
														getIndexedItemFilePath(item);

													if (!filePathData) return;

													const locationId = filePathData.location_id;

													if (locationId === null) return;

													renameFile.mutate({
														location_id: locationId,
														kind: {
															One: {
																from_file_path_id: item.item.id,
																to: newName
															}
														}
													});

													setNewName(newName);
												}}
											/>
										) : (
											<Tooltip label={name} className="truncate">
												<span
													onClick={() =>
														name &&
														item.type !== 'NonIndexedPath' &&
														setIsRenaming(true)
													}
													className={clsx(
														item.type === 'NonIndexedPath'
															? 'cursor-default'
															: 'cursor-text'
													)}
												>
													{name}
												</span>
											</Tooltip>
										)}
									</div>

									<div className="flex flex-1 justify-end gap-1">
										<DropdownMenu.Root
											trigger={
												<div className="flex">
													<Tooltip label="More">
														<IconButton>
															<DotsThree size={20} weight="bold" />
														</IconButton>
													</Tooltip>
												</div>
											}
											onOpenChange={setIsContextMenuOpen}
											align="end"
											sideOffset={-10}
										>
											<ExplorerContextMenu items={[item]} custom>
												<Conditional
													items={[
														SharedItems.OpenOrDownload,
														SharedItems.RevealInNativeExplorer
													]}
												/>

												{item.type !== 'NonIndexedPath' && (
													<DropdownMenu.Item
														label="Rename"
														onClick={() => name && setIsRenaming(true)}
													/>
												)}

												<SeparatedConditional
													items={[ObjectItems.AssignTag]}
												/>

												<Conditional
													items={[
														FilePathItems.CopyAsPath,
														FilePathItems.Crypto,
														FilePathItems.Compress,
														ObjectItems.ConvertObject,
														FilePathItems.SecureDelete
													]}
												>
													{(items) => (
														<DropdownMenu.SubMenu
															label="More actions..."
															icon={Plus}
														>
															{items}
														</DropdownMenu.SubMenu>
													)}
												</Conditional>

												<SeparatedConditional
													items={[FilePathItems.Delete]}
												/>
											</ExplorerContextMenu>
										</DropdownMenu.Root>

										<Tooltip label="Show details">
											<IconButton
												onClick={() => setShowMetadata(!showMetadata)}
												active={showMetadata}
											>
												<SidebarSimple
													className="rotate-180"
													weight={showMetadata ? 'fill' : 'regular'}
												/>
											</IconButton>
										</Tooltip>
									</div>
								</div>

								<FileThumb
									data={item}
									onLoad={(type) =>
										type === 'ORIGINAL' && setThumbErrorToast(undefined)
									}
									onError={(type, error) =>
										type === 'ORIGINAL' &&
										setThumbErrorToast({
											title: 'Error loading original file',
											body: error.message
										})
									}
									loadOriginal
									mediaControls
									className={clsx(
										'm-3 !w-auto flex-1 !overflow-hidden rounded',
										!background && !icon && 'bg-app-box shadow'
									)}
									childClassName={clsx(
										'rounded',
										kind === 'Text' && 'p-3',
										!icon && 'h-full',
										textKinds.includes(kind) && 'select-text'
									)}
								/>
							</div>

							{showMetadata && (
								<div className="no-scrollbar w-64 shrink-0 border-l border-app-line bg-app-darkBox py-1">
									<SingleItemMetadata item={item} />
								</div>
							)}
						</div>
					</Dialog.Content>
				</Dialog.Portal>
			</QuickPreviewContext.Provider>
		</Dialog.Root>
	);
};

interface RenameInputProps {
	name: string;
	onRename: (name: string) => void;
}

const RenameInput = ({ name, onRename }: RenameInputProps) => {
	const isDark = useIsDark();

	const os = useOperatingSystem();

	const quickPreview = useQuickPreviewContext();

	const _ref = useRef<HTMLInputElement | null>(null);

	const form = useZodForm({ schema: z.object({ name: z.string() }), defaultValues: { name } });

	const onSubmit = form.handleSubmit(({ name }) => onRename(name));

	const { ref, ...register } = form.register('name', {
		onBlur: onSubmit
	});

	const highlightName = useCallback(() => {
		const endRange = name.lastIndexOf('.');
		setTimeout(() => _ref.current?.setSelectionRange(0, endRange || name.length));
	}, [name]);

	const handleKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
		e.stopPropagation();

		switch (e.key) {
			case 'Tab': {
				e.preventDefault();
				onSubmit();
				break;
			}

			case 'Escape': {
				form.reset();
				onSubmit();
				break;
			}

			case 'z': {
				if (os === 'macOS' ? e.metaKey : e.ctrlKey) {
					form.reset();
					highlightName();
				}
			}
		}
	};

	useEffect(() => {
		if (document.activeElement !== _ref.current) highlightName();
	}, [highlightName]);

	return (
		<Form form={form} onSubmit={onSubmit} className="w-1/2">
			<input
				autoFocus
				autoCorrect="off"
				className={clsx(
					'w-full rounded border px-2 py-1 text-center outline-none',
					quickPreview.background
						? 'border-white/[.12] bg-white/10 backdrop-blur-sm'
						: isDark
						? 'border-app-line bg-app-input'
						: 'border-black/[.075] bg-black/[.075]'
				)}
				onKeyDown={handleKeyDown}
				onFocus={() => highlightName()}
				ref={(e) => {
					ref(e);
					_ref.current = e;
				}}
				{...register}
			/>
		</Form>
	);
};

const IconButton = ({
	className,
	active,
	...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { active?: boolean }) => {
	const isDark = useIsDark();

	const quickPreview = useQuickPreviewContext();

	return (
		<button
			className={clsx(
				'text-md inline-flex h-[30px] w-[30px] items-center justify-center rounded opacity-80 outline-none backdrop-blur-none',
				'hover:opacity-100 hover:backdrop-blur',
				'focus:opacity-100 focus:backdrop-blur',
				'disabled:pointer-events-none disabled:opacity-40',
				isDark || quickPreview.background
					? quickPreview.background
						? 'hover:bg-white/[.15] focus:bg-white/[.15]'
						: 'hover:bg-app-box focus:bg-app-box'
					: 'hover:bg-black/[.075] focus:bg-black/[.075]',
				active && [
					'!opacity-100 backdrop-blur',
					isDark || quickPreview.background
						? quickPreview.background
							? 'bg-white/[.15]'
							: 'bg-app-box'
						: 'bg-black/[.075]'
				],
				className
			)}
			{...props}
		/>
	);
};
