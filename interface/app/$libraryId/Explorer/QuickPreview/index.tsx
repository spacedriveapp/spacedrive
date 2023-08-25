import * as Dialog from '@radix-ui/react-dialog';
import { animated, useTransition } from '@react-spring/web';
import clsx from 'clsx';
import { ArrowLeft, ArrowRight, CaretDown, Plus, SidebarSimple, X } from 'phosphor-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import {
	type ExplorerItem,
	getExplorerItemData,
	getIndexedItemFilePath,
	getItemFilePath,
	useLibraryContext,
	useLibraryMutation,
	useRspcLibraryContext
} from '@sd/client';
import {
	DropdownMenu,
	Form,
	ModifierKeys,
	Tooltip,
	dialogManager,
	toast,
	tw,
	useZodForm,
	z
} from '@sd/ui';
import { useIsDark, useOperatingSystem } from '~/hooks';
import { useKeyBind } from '~/hooks/useKeyBind';
import { usePlatform } from '~/util/Platform';
import { useExplorerContext } from '../Context';
import ExplorerContextMenu, { FilePathItems, ObjectItems, SharedItems } from '../ContextMenu';
import { Conditional } from '../ContextMenu/ConditionalItem';
import DeleteDialog from '../FilePath/DeleteDialog';
import { FileThumb } from '../FilePath/Thumb';
import { SingleItemMetadata } from '../Inspector';
import { getExplorerStore, useExplorerStore } from '../store';

const AnimatedDialogOverlay = animated(Dialog.Overlay);
const AnimatedDialogContent = animated(Dialog.Content);

const ArrowButton = tw.button`flex h-9 w-9 shrink-0 items-center shadow p-2 cursor-pointer justify-center rounded-full border border-app-line bg-app/80 text-ink/80 text-xl`;
const IconButton = tw.button`inline-flex h-8 w-8 items-center justify-center rounded-md text-md text-slate-300 hover:bg-white/10 hover:text-white hover:backdrop-blur outline-none`;

const fadeInClassName =
	'opacity-0 group-focus-within:opacity-100 group-hover:opacity-100 animate-in fade-in fade-out duration-300';

export const QuickPreview = () => {
	const rspc = useRspcLibraryContext();
	const isDark = useIsDark();
	const os = useOperatingSystem();
	const { library } = useLibraryContext();
	const { openFilePaths, revealItems } = usePlatform();

	const explorer = useExplorerContext();
	const { showQuickView } = useExplorerStore();

	const [selectedItems, setSelectedItems] = useState<ExplorerItem[]>([]);
	const [currentItemIndex, setCurrentItemIndex] = useState(0);
	const [showMetadata, setShowMetadata] = useState<boolean>(false);
	const [isContextMenuOpen, setIsContextMenuOpen] = useState<boolean>(false);
	const [isRenaming, setIsRenaming] = useState<boolean>(false);
	const [newName, setNewName] = useState<string | null>(null);

	const item = useMemo(() => selectedItems[currentItemIndex], [selectedItems, currentItemIndex]);

	const transitions = useTransition(showQuickView, {
		from: {
			opacity: 0,
			transform: `translateY(20px) scale(0.9)`,
			transformOrigin: 'center top'
		},
		enter: { opacity: 1, transform: `translateY(0px) scale(1)` },
		leave: { opacity: 0, transform: `translateY(40px) scale(0.9)` },
		config: { mass: 0.2, tension: 300, friction: 20, bounce: 0 }
	});

	const renameFile = useLibraryMutation(['files.renameFile'], {
		onError: () => setNewName(null),
		onSuccess: () => rspc.queryClient.invalidateQueries(['search.paths'])
	});

	const changeCurrentItem = (index: number) => {
		if (selectedItems[index]) {
			setCurrentItemIndex(index);
			setNewName(null);
		}
	};

	// Update items
	useEffect(() => {
		if (showQuickView) {
			if (explorer.selectedItems.size === 0) getExplorerStore().showQuickView = false;
			else setSelectedItems([...explorer.selectedItems]);
		}
	}, [explorer.selectedItems, showQuickView]);

	// Reset state
	useEffect(() => {
		if (showQuickView) {
			setCurrentItemIndex(0);
			setShowMetadata(false);
			setNewName(null);
		}
	}, [showQuickView]);

	// Toggle quick preview
	useKeyBind(['space'], (e) => {
		if (isRenaming) return;

		e.preventDefault();

		getExplorerStore().showQuickView = !showQuickView;
	});

	// Move between items
	useKeyBind([['left'], ['right']], (e) => {
		if (isContextMenuOpen || isRenaming) return;
		changeCurrentItem(e.key === 'ArrowLeft' ? currentItemIndex - 1 : currentItemIndex + 1);
	});

	// Toggle metadata
	useKeyBind([os === 'macOS' ? ModifierKeys.Meta : ModifierKeys.Control, 'i'], () =>
		setShowMetadata(!showMetadata)
	);

	// Open file
	useKeyBind([os === 'macOS' ? ModifierKeys.Meta : ModifierKeys.Control, 'o'], () => {
		if (!item || !openFilePaths) return;

		try {
			const path = getIndexedItemFilePath(item);

			if (!path) throw 'No path found';

			openFilePaths(library.uuid, [path.id]);
		} catch (error) {
			toast.error({
				title: 'Failed to open file',
				description: `Couldn't open file, due to an error: ${error}`
			});
		}
	});

	// Reveal in native explorer
	useKeyBind([os === 'macOS' ? ModifierKeys.Meta : ModifierKeys.Control, 'y'], () => {
		if (!item || !revealItems) return;

		try {
			const id = item.type === 'Location' ? item.item.id : getIndexedItemFilePath(item)?.id;

			if (!id) throw 'No id found';

			revealItems(library.uuid, [
				{ ...(item.type === 'Location' ? { Location: { id } } : { FilePath: { id } }) }
			]);
		} catch (error) {
			toast.error({
				title: 'Failed to reveal',
				description: `Couldn't reveal file, due to an error: ${error}`
			});
		}
	});

	// Open delete dialog
	useKeyBind([os === 'macOS' ? ModifierKeys.Meta : ModifierKeys.Control, 'backspace'], () => {
		if (!item) return;

		const path = getIndexedItemFilePath(item);

		if (!path || path.location_id === null) return;

		dialogManager.create((dp) => (
			<DeleteDialog {...dp} locationId={path.location_id!} pathIds={[path.id]} />
		));
	});

	return (
		<Dialog.Root
			open={showQuickView}
			onOpenChange={(open) => (getExplorerStore().showQuickView = open)}
		>
			{transitions((styles, show) => {
				if (!show || !item) return null;

				const filePathData = getItemFilePath(item);

				if (!filePathData) return null;

				const { kind } = getExplorerItemData(item);

				const fixedHeader = kind === 'Text' || kind === 'Document';

				const name = `${filePathData.name}${
					filePathData.extension ? `.${filePathData.extension}` : ''
				}`;

				return (
					<Dialog.Portal forceMount>
						<AnimatedDialogOverlay
							style={{
								opacity: styles.opacity
							}}
							className={clsx(
								'absolute inset-0 z-50',
								isDark ? 'bg-black/75' : 'bg-black/60'
							)}
							onContextMenu={(e) => {
								// Block explorer context menu
								// should probably look for a better solution
								e.stopPropagation();
								e.preventDefault();
							}}
						/>

						<AnimatedDialogContent
							style={styles}
							className={clsx(
								'fixed inset-8 z-50 flex overflow-hidden rounded-md border outline-none backdrop-blur',
								isDark
									? 'border-app-line/80'
									: fixedHeader
									? 'border-app-line'
									: 'border-app-line/10'
							)}
							onOpenAutoFocus={(e) => e.preventDefault()}
							onEscapeKeyDown={(e) => isRenaming && e.preventDefault()}
							onContextMenu={(e) => {
								// Block explorer context menu
								// should probably look for a better solution
								e.stopPropagation();
								e.preventDefault();
							}}
						>
							<div
								className={clsx(
									'group relative flex flex-1 flex-col overflow-hidden',
									fixedHeader ? 'bg-app' : 'bg-app/80'
								)}
							>
								<FileThumb
									data={item}
									cover={true}
									className={(type) =>
										clsx(
											'!absolute inset-0',
											kind !== 'Text' && type !== 'ICON' && 'bg-black'
										)
									}
									childClassName={(type) =>
										type === 'ICON' ? 'hidden' : 'opacity-30 blur-md'
									}
								/>

								<div
									className={clsx(
										'z-50 flex items-center p-3 ',
										fixedHeader
											? 'border-b border-app-line bg-app-darkBox text-ink'
											: [
													'absolute inset-x-0 top-0 border-none bg-gradient-to-b from-black/50 to-transparent text-white',
													fadeInClassName
											  ]
									)}
								>
									<div className="flex-1">
										<Tooltip label="Close" hoverable={false}>
											<Dialog.Close asChild>
												<IconButton>
													<X weight="bold" />
												</IconButton>
											</Dialog.Close>
										</Tooltip>
									</div>

									<div className="flex w-1/2 items-center justify-center truncate text-sm">
										{isRenaming ? (
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
											<>
												<span
													onClick={() =>
														'id' in item.item && setIsRenaming(true)
													}
													className="truncate"
												>
													{newName || name}
												</span>
												<DropdownMenu.Root
													trigger={
														<CaretDown
															size={16}
															weight="bold"
															className="ml-2 shrink-0 cursor-pointer transition-all hover:mt-1 radix-state-open:mt-1"
														/>
													}
													onOpenChange={setIsContextMenuOpen}
													usePortal={false}
													modal={false}
													align="center"
												>
													<ExplorerContextMenu items={[item]} custom>
														<Conditional
															items={[
																FilePathItems.OpenOrDownload,
																SharedItems.RevealInNativeExplorer
															]}
														/>
														<DropdownMenu.Item
															label="Rename"
															onClick={() => setIsRenaming(true)}
														/>
														<DropdownMenu.Separator />
														<Conditional
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
														<DropdownMenu.Separator />
														<Conditional
															items={[FilePathItems.Delete]}
														/>
													</ExplorerContextMenu>
												</DropdownMenu.Root>
											</>
										)}
									</div>

									<div className="flex flex-1 justify-end">
										<Tooltip label="Show details" hoverable={false}>
											<IconButton
												onClick={() => setShowMetadata(!showMetadata)}
												className={clsx(
													showMetadata &&
														'bg-white/10 !text-white !backdrop-blur'
												)}
											>
												<SidebarSimple className="rotate-180" />
											</IconButton>
										</Tooltip>
									</div>
								</div>

								<Navigation
									showBack={currentItemIndex - 1 >= 0}
									showForward={selectedItems.length > currentItemIndex + 1}
									onNav={(val) => changeCurrentItem(currentItemIndex + val)}
								/>

								<FileThumb
									data={item}
									loadOriginal
									mediaControls
									className={clsx(
										fixedHeader && '!h-auto flex-1 overflow-hidden'
									)}
									childClassName={clsx(kind === 'Text' && 'p-6')}
								/>
							</div>

							{showMetadata && (
								<div className="no-scrollbar w-64 shrink-0 border-l border-app-line bg-app-darkBox py-1">
									<SingleItemMetadata item={item} />
								</div>
							)}
						</AnimatedDialogContent>
					</Dialog.Portal>
				);
			})}
		</Dialog.Root>
	);
};

interface NavigationProps {
	showBack: boolean;
	showForward: boolean;
	onNav: (val: number) => void;
}

const Navigation = ({ showBack, showForward, onNav }: NavigationProps) => {
	return (
		<>
			{showBack && (
				<Tooltip label="Previous" className="absolute left-6 top-1/2 z-50 -translate-y-1/2">
					<ArrowButton onClick={() => onNav(-1)} className={fadeInClassName}>
						<ArrowLeft weight="bold" />
					</ArrowButton>
				</Tooltip>
			)}

			{showForward && (
				<Tooltip label="Next" className="absolute right-6 top-1/2 z-50 -translate-y-1/2">
					<ArrowButton onClick={() => onNav(1)} className={fadeInClassName}>
						<ArrowRight weight="bold" />
					</ArrowButton>
				</Tooltip>
			)}
		</>
	);
};

interface RenameInputProps {
	name: string;
	onRename: (name: string) => void;
	className?: string;
}

const RenameInput = ({ name, onRename, className }: RenameInputProps) => {
	const os = useOperatingSystem();

	const _ref = useRef<HTMLInputElement | null>(null);

	const form = useZodForm({ schema: z.object({ name: z.string() }), defaultValues: { name } });

	const onSubmit = form.handleSubmit(({ name }) => onRename(name));

	const { ref, ...register } = form.register('name', {
		onBlur: onSubmit
	});

	const highlightName = (name: string) => {
		const endRange = name.lastIndexOf('.');
		setTimeout(() => _ref.current?.setSelectionRange(0, endRange || name.length));
	};

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
					highlightName(name);
				}
			}
		}
	};

	return (
		<Form form={form} onSubmit={onSubmit} className="w-1/2">
			<input
				autoFocus
				autoCorrect="off"
				className={clsx(
					'w-full rounded border border-white/[.12] bg-white/10 px-2 py-1 text-center outline-none backdrop-blur-sm',
					className
				)}
				onKeyDown={handleKeyDown}
				onFocus={() => highlightName(name)}
				ref={(e) => {
					ref(e);
					_ref.current = e;
				}}
				{...register}
			/>
		</Form>
	);
};
