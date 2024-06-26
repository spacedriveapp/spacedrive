import {
	ArrowLeft,
	ArrowRight,
	DotsThree,
	MagnifyingGlassMinus,
	MagnifyingGlassPlus,
	Plus,
	SidebarSimple,
	Slideshow,
	X
} from '@phosphor-icons/react';
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
import { useKey } from 'rooks';
import {
	ExplorerItem,
	getEphemeralPath,
	getExplorerItemData,
	getIndexedItemFilePath,
	ObjectKindKey,
	useExplorerLayoutStore,
	useLibraryContext,
	useLibraryMutation,
	useRspcLibraryContext,
	useZodForm
} from '@sd/client';
import { DropdownMenu, Form, toast, ToastMessage, Tooltip, z } from '@sd/ui';
import { useIsDark, useLocale, useOperatingSystem, useShortcut } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { useExplorerContext } from '../Context';
import ExplorerContextMenu, {
	FilePathItems,
	ObjectItems,
	SeparatedConditional,
	SharedItems
} from '../ContextMenu';
import { Conditional } from '../ContextMenu/ConditionalItem';
import { FileThumb } from '../FilePath/Thumb';
import { SingleItemMetadata } from '../Inspector';
import { explorerStore } from '../store';
import { useExplorerViewContext } from '../View/Context';
import { ImageSlider } from './ImageSlider';
import { getQuickPreviewStore, useQuickPreviewStore } from './store';

export type QuickPreviewItem = { item: ExplorerItem; index: number };

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
	const rspc = useRspcLibraryContext();
	const isDark = useIsDark();
	const { library } = useLibraryContext();
	const { openFilePaths, openEphemeralFiles } = usePlatform();
	const explorerLayoutStore = useExplorerLayoutStore();
	const explorer = useExplorerContext();
	const explorerView = useExplorerViewContext();
	const { open, itemIndex } = useQuickPreviewStore();

	const thumb = createRef<HTMLDivElement>();
	const [thumbErrorToast, setThumbErrorToast] = useState<ToastMessage>();
	const [showMetadata, setShowMetadata] = useState<boolean>(false);
	const [magnification, setMagnification] = useState<number>(1);
	const [isContextMenuOpen, setIsContextMenuOpen] = useState<boolean>(false);
	const [isRenaming, setIsRenaming] = useState<boolean>(false);
	const [newName, setNewName] = useState<string | null>(null);

	const { t } = useLocale();

	const items = useMemo(() => {
		if (!open || !explorer.items || explorer.selectedItems.size === 0) return [];

		const items: QuickPreviewItem[] = [];

		// Sort selected items
		for (let i = 0; i < explorer.items.length; i++) {
			const item = explorer.items[i];
			if (!item) continue;

			if (explorer.selectedItems.has(item)) items.push({ item, index: i });
			if (items.length === explorer.selectedItems.size) break;
		}

		return items;
	}, [explorer.items, explorer.selectedItems, open]);

	const item = useMemo(() => items[itemIndex]?.item ?? null, [items, itemIndex]);

	const activeItem = items[itemIndex];

	const renameFile = useLibraryMutation(['files.renameFile'], {
		onError: () => setNewName(null),
		onSuccess: () => rspc.queryClient.invalidateQueries(['search.paths'])
	});

	const renameEphemeralFile = useLibraryMutation(['ephemeralFiles.renameFile'], {
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
		setMagnification(1);

		if (open || item) return;

		getQuickPreviewStore().open = false;
		getQuickPreviewStore().itemIndex = 0;
		setShowMetadata(false);
	}, [item, open]);

	useEffect(() => {
		if (open) explorerView.updateActiveItem(null, { updateFirstItem: true });

		// "open" is excluded, as we only want this to trigger when hashes change,
		// that way we don't have to manually update the active item.
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [explorer.selectedItemHashes, explorerView.updateActiveItem]);

	const handleMoveBetweenItems = (step: number) => {
		const nextPreviewItem = items[itemIndex + step];
		if (nextPreviewItem) {
			getQuickPreviewStore().itemIndex = itemIndex + step;
			return;
		}

		if (!activeItem || !explorer.items) return;
		if (items.length > 1 && !explorerLayoutStore.showImageSlider) return;

		const newSelectedItem =
			items.length > 1 &&
			(activeItem.index === 0 || activeItem.index === explorer.items.length - 1)
				? activeItem.item
				: explorer.items[activeItem.index + step];

		if (!newSelectedItem) return;

		explorer.resetSelectedItems([newSelectedItem]);
		getQuickPreviewStore().itemIndex = 0;
	};

	useShortcut('quickPreviewMoveBack', () => {
		if (isContextMenuOpen || isRenaming) return;
		handleMoveBetweenItems(-1);
	});

	useShortcut('quickPreviewMoveForward', () => {
		if (isContextMenuOpen || isRenaming) return;
		handleMoveBetweenItems(1);
	});

	useKey('ArrowDown', () => {
		if (items.length < 2 || !activeItem) return;
		explorer.resetSelectedItems([activeItem.item]);
		getQuickPreviewStore().itemIndex = 0;
	});

	//close quick preview
	useShortcut('closeQuickPreview', (e) => {
		if (explorerStore.isCMDPOpen) return;
		e.preventDefault();
		getQuickPreviewStore().open = false;
	});

	// Toggle metadata
	useShortcut('toggleMetaData', () => setShowMetadata(!showMetadata));

	// Open file
	useShortcut('quickPreviewOpenNative', () => {
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
				title: t('failed_to_open_file_title'),
				body: t('failed_to_open_file_body', { error: error })
			});
		}
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
							'absolute inset-0 z-[100]',
							'radix-state-open:animate-in radix-state-open:fade-in-0',
							isDark ? 'bg-black/80' : 'bg-black/60'
						)}
						onContextMenu={(e) => e.preventDefault()}
					/>

					<Dialog.Content
						className="fixed inset-[5%] z-[100] outline-none radix-state-open:animate-in radix-state-open:fade-in-0 radix-state-open:zoom-in-95"
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
										<div className="absolute inset-0 bg-black/50 backdrop-blur-3xl" />
									</div>
								)}
								<div
									className={clsx(
										'z-50 flex items-center p-2',
										background ? 'text-white' : 'text-ink'
									)}
								>
									<div className="flex flex-1">
										<Tooltip label={t('close')}>
											<Dialog.Close asChild>
												<IconButton>
													<X weight="bold" />
												</IconButton>
											</Dialog.Close>
										</Tooltip>

										{items.length > 1 && (
											<div className="ml-2 flex">
												<Tooltip label={t('back')}>
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

												<Tooltip label={t('forward')}>
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

													if (!newName || newName === name) return;

													try {
														switch (item.type) {
															case 'Path':
															case 'Object': {
																const filePathData =
																	getIndexedItemFilePath(item);

																if (!filePathData)
																	throw new Error(
																		'Failed to get file path object'
																	);

																const { id, location_id } =
																	filePathData;

																if (!location_id)
																	throw new Error(
																		'Missing location id'
																	);

																renameFile.mutate({
																	location_id,
																	kind: {
																		One: {
																			from_file_path_id: id,
																			to: newName
																		}
																	}
																});

																break;
															}
															case 'NonIndexedPath': {
																const ephemeralFile =
																	getEphemeralPath(item);

																if (!ephemeralFile)
																	throw new Error(
																		'Failed to get ephemeral file object'
																	);

																renameEphemeralFile.mutate({
																	kind: {
																		One: {
																			from_path:
																				ephemeralFile.path,
																			to: newName
																		}
																	}
																});

																break;
															}

															default:
																throw new Error(
																	'Invalid explorer item type'
																);
														}

														setNewName(newName);
													} catch (e) {
														toast.error({
															title: t('failed_to_rename_file', {
																oldName: itemData.fullName,
																newName
															}),
															body: t('error_message', { error: e })
														});
													}
												}}
											/>
										) : (
											<Tooltip label={name} className="truncate">
												<span
													onClick={() => name && setIsRenaming(true)}
													className={clsx('cursor-text')}
												>
													{name}
												</span>
											</Tooltip>
										)}
									</div>

									<div className="flex flex-1 items-center justify-end gap-1">
										<Tooltip label={t('zoom_in')}>
											<IconButton
												onClick={() => {
													magnification < 2 &&
														setMagnification(
															(currentMagnification) =>
																currentMagnification +
																currentMagnification * 0.2
														);
												}}
												// this is same formula as interest calculation
											>
												<MagnifyingGlassPlus />
											</IconButton>
										</Tooltip>

										<Tooltip label={t('zoom_out')}>
											<IconButton
												onClick={() => {
													magnification > 0.5 &&
														setMagnification(
															(currentMagnification) =>
																currentMagnification / (1 + 0.2)
														);
												}}
												// this is same formula as interest calculation
											>
												<MagnifyingGlassMinus />
											</IconButton>
										</Tooltip>

										<DropdownMenu.Root
											trigger={
												<div className="flex">
													<Tooltip label={t('more')}>
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

												<DropdownMenu.Item
													label={t('rename')}
													onClick={() => name && setIsRenaming(true)}
												/>

												<SeparatedConditional
													items={[ObjectItems.AssignTag]}
												/>

												<Conditional
													items={[
														FilePathItems.CopyAsPath,
														FilePathItems.Crypto,
														FilePathItems.Compress,
														ObjectItems.ConvertObject
														// FilePathItems.SecureDelete
													]}
												>
													{(items) => (
														<DropdownMenu.SubMenu
															label={t('more_actions')}
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

										<Tooltip label={t('show_slider')}>
											<IconButton
												onClick={() =>
													(explorerLayoutStore.showImageSlider =
														!explorerLayoutStore.showImageSlider)
												}
												className="w-fit px-2 text-[10px]"
											>
												<Slideshow
													size={16.5}
													weight={
														explorerLayoutStore.showImageSlider
															? 'fill'
															: 'regular'
													}
												/>
											</IconButton>
										</Tooltip>

										<Tooltip label={t('show_details')}>
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
										type.variant === 'original' && setThumbErrorToast(undefined)
									}
									onError={(type, error) =>
										type.variant === 'original' &&
										setThumbErrorToast({
											title: t('error_loading_original_file'),
											body: error.message
										})
									}
									loadOriginal
									frameClassName="!border-0"
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
									magnification={magnification}
								/>

								{explorerLayoutStore.showImageSlider && activeItem && (
									<ImageSlider activeItem={activeItem} />
								)}
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
				'text-md inline-flex size-[30px] items-center justify-center rounded opacity-80 outline-none',
				'hover:opacity-100',
				'focus:opacity-100',
				'disabled:pointer-events-none disabled:opacity-40',
				isDark || quickPreview.background
					? quickPreview.background
						? 'hover:bg-white/[.15] focus:bg-white/[.15]'
						: 'hover:bg-app-box focus:bg-app-box'
					: 'hover:bg-black/[.075] focus:bg-black/[.075]',
				active && [
					'!opacity-100',
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
