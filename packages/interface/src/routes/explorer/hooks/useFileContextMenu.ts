import {
	ArrowSquareOut,
	ArrowClockwise,
	Books,
	Browsers,
	Copy,
	Crop,
	Eye,
	FileText,
	FileVideo,
	FilmStrip,
	FolderOpen,
	FolderPlus,
	Image,
	MagnifyingGlass,
	Microphone,
	Pencil,
	Scissors,
	ShareNetwork,
	Sparkle,
	Stack,
	Tag as TagIconComponent,
	TextAa,
	Trash,
	Video,
	Waveform
} from '@phosphor-icons/react';
import type {File} from '@sd/ts-client';
import {getContentKind, isVirtualFile} from '@sd/ts-client';
import { toast } from '@spacedrive/primitives';
import {useFileOperationDialog} from '../../../components/modals/FileOperationModal';
import {usePlatform} from '../../../contexts/PlatformContext';
import {useLibraryMutation} from '../../../contexts/SpacedriveContext';
import {useClipboard} from '../../../hooks/useClipboard';
import {useContextMenu} from '../../../hooks/useContextMenu';
import {useOpenWith} from '../../../hooks/useOpenWith';
import {useRefetchTagQueries} from '../../../hooks/useRefetchTagQueries';
import {useTabManager} from '../../../components/TabManager';
import {targetToUrl, useExplorer} from '../context';
import {useSelection} from '../SelectionContext';
import {useDeleteFiles} from './useDeleteFiles';

interface UseFileContextMenuProps {
	file?: File | null;
	selectedFiles: File[];
	selected: boolean;
}

export function useFileContextMenu({
	file,
	selectedFiles,
	selected
}: UseFileContextMenuProps) {
	const {navigateToPath, currentPath, mode} = useExplorer();
	const {createTab} = useTabManager();
	const platform = usePlatform();
	const refetchTagQueries = useRefetchTagQueries();

	const {deleteFiles} = useDeleteFiles();
	const unapplyTags = useLibraryMutation('tags.unapply', {
		onSuccess: refetchTagQueries
	});
	const createFolder = useLibraryMutation('files.createFolder');
	const addToLibrary = useLibraryMutation('sources.track');
	const generateThumbnails = useLibraryMutation('thumbs.generate');

	// Helper to run a mutation on each target file
	const forEachTarget = async (
		targets: File[],
		fn: (f: File) => Promise<unknown>
	) => {
		for (const f of targets) {
			try {
				await fn(f);
			} catch (err) {
				console.error(`Failed for ${f.name}:`, err);
			}
		}
	};
	const clipboard = useClipboard();
	const openFileOperation = useFileOperationDialog();
	const {startRename} = useSelection();

	// Get physical paths for file opening
	const getPhysicalPaths = () => {
		const targets =
			selected && selectedFiles.length > 0 ? selectedFiles : [file];
		return targets
			.filter((f): f is File => f != null && f.sd_path != null && 'Physical' in f.sd_path)
			.map((f) => (f.sd_path as any).Physical.path);
	};

	const physicalPaths = getPhysicalPaths();
	const {apps, openWithDefault, openWithApp, openMultipleWithApp} =
		useOpenWith(physicalPaths);

	// Get the files to operate on (multi-select or just this file)
	// Filters out virtual files (they're display-only, not real filesystem entries)
	const getTargetFiles = () => {
		const targets =
			selected && selectedFiles.length > 0 ? selectedFiles : [file];
		// Filter out virtual files - they cannot be copied/moved/deleted
		return targets.filter((f): f is File => f != null && !isVirtualFile(f));
	};

	// Check if any selected files are virtual (to disable certain operations)
	const hasVirtualFiles = selected
		? selectedFiles.some((f) => isVirtualFile(f))
		: file
			? isVirtualFile(file)
			: false;

	return useContextMenu({
		items: [
			{
				icon: ArrowClockwise,
				label: 'Regenerate Thumbnails',
				condition: () => getTargetFiles().some((target) => 'Physical' in target.sd_path),
				onClick: async () => {
					try {
						for (const target of getTargetFiles()) {
							if (!('Physical' in target.sd_path)) continue;
							await generateThumbnails.mutateAsync({
								scope: target.sd_path,
								recursive: target.kind === 'Directory',
								mode: 'force'
							});
						}
						toast.success('Thumbnail regeneration started');
					} catch (error) {
						toast.error(`Could not regenerate thumbnails: ${error}`);
					}
				}
			},
			{
				icon: Eye,
				label: 'Quick Look',
				onClick: () => {
					if (!file) return;
					console.log('Quick Look:', file.name);
					// TODO: Implement quick look
				},
				keybind: 'Space',
				condition: () => !!file
			},
			{
				icon: FolderOpen,
				label: 'Open',
				onClick: async () => {
					if (!file) return;
					if (file.kind === 'Directory') {
						navigateToPath(file.sd_path);
					} else if ('Physical' in file.sd_path) {
						const physicalPath = (file.sd_path as any).Physical
							.path;
						await openWithDefault(physicalPath);
					}
				},
				keybind: '⌘O',
				condition: () =>
					!!file &&
					(file.kind === 'Directory' || file.kind === 'File')
			},
			{
				icon: Browsers,
				label: 'Open in New Tab',
				onClick: () => {
					if (!file) return;
					createTab(
						file.name,
						targetToUrl({type: 'path', path: file.sd_path})
					);
				},
				condition: () => !!file && file.kind === 'Directory'
			},
			{
				type: 'submenu',
				icon: ArrowSquareOut,
				label: 'Open With',
				condition: () =>
					!!file &&
					file.kind === 'File' &&
					'Physical' in file.sd_path &&
					apps.length > 0,
				submenu: apps.map((app) => ({
					label: app.name,
					onClick: async () => {
						if (!file) return;
						if (selected && selectedFiles.length > 1) {
							await openMultipleWithApp(physicalPaths, app.id);
						} else if ('Physical' in file.sd_path) {
							const physicalPath = (file.sd_path as any).Physical
								.path;
							await openWithApp(physicalPath, app.id);
						}
					}
				}))
			},
			{
				icon: MagnifyingGlass,
				label: 'Show in Finder',
				onClick: async () => {
					if (!file) return;
					// Extract the physical path from SdPath
					if ('Physical' in file.sd_path) {
						const physicalPath = file.sd_path.Physical.path;
						if (platform.revealFile) {
							try {
								await platform.revealFile(physicalPath);
							} catch (err) {
								console.error('Failed to reveal file:', err);
								toast.error(`Failed to reveal file: ${err}`);
							}
						} else {
							console.log(
								'revealFile not supported on this platform'
							);
						}
					} else {
						console.log('Cannot reveal non-physical file');
					}
				},
				keybind: '⌘⇧R',
				condition: () =>
					!!file &&
					'Physical' in file.sd_path &&
					!!platform.revealFile
			},
			{
				icon: ShareNetwork,
				label:
					selected && selectedFiles.length > 1
						? `Share ${selectedFiles.length} items`
						: 'Share',
				onClick: async () => {
					const paths = physicalPaths;
					if (paths.length === 0) {
						console.warn('No physical files to share');
						return;
					}
					if (platform.shareFiles) {
						try {
							await platform.shareFiles(paths);
						} catch (err) {
							console.error('Failed to share files:', err);
							toast.error(`Failed to share: ${err}`);
						}
					}
				},
				condition: () =>
					physicalPaths.length > 0 && !!platform.shareFiles
			},
			{
				icon: Books,
				label: 'Add to Library',
				// Everything on the drive is already searchable. What this adds
				// is what a walk cannot rebuild: tags and notes that stay put,
				// files still listed when the drive is unplugged, and sync to
				// another device.
				onClick: async () => {
					if (!file || !('Physical' in file.sd_path)) return;
					const path = file.sd_path.Physical.path;
					try {
						await addToLibrary.mutateAsync({
							path,
							name: null,
							unfiltered: false
						});
						toast.success(`Added ${file.name} to your library`);
					} catch (err) {
						console.error('Failed to add to library:', err);
						toast.error(`Could not add ${file.name}: ${err}`);
					}
				},
				condition: () =>
					!!file &&
					file.kind === 'Directory' &&
					selectedFiles.length === 1 &&
					'Physical' in file.sd_path &&
					!hasVirtualFiles
			},
			{type: 'separator'},
			{
				icon: Pencil,
				label: 'Rename',
				onClick: () => {
					if (!file) return;
					startRename(file.id);
				},
				keybindId: 'explorer.renameFile',
				condition: () =>
					!!file &&
					selected &&
					selectedFiles.length === 1 &&
					!hasVirtualFiles
			},
			{
				icon: FolderPlus,
				label: 'New Folder',
				onClick: async () => {
					if (!currentPath) return;
					try {
						const result = await createFolder.mutateAsync({
							parent: currentPath,
							name: 'Untitled Folder',
							items: []
						});
						console.log('Created folder:', result);
					} catch (err) {
						console.error('Failed to create folder:', err);
						toast.error(`Failed to create folder: ${err}`);
					}
				},
				condition: () => !!currentPath
			},
			{
				icon: FolderPlus,
				label: 'New Folder with Items',
				onClick: async () => {
					if (!currentPath) return;
					const targets = getTargetFiles();
					if (targets.length === 0) return;

					try {
						const result = await createFolder.mutateAsync({
							parent: currentPath,
							name: 'New Folder',
							items: targets.map((f) => f.sd_path)
						});
						console.log('Created folder with items:', result);
					} catch (err) {
						console.error(
							'Failed to create folder with items:',
							err
						);
						toast.error(`Failed to create folder: ${err}`);
					}
				},
				condition: () =>
					!!currentPath &&
					selectedFiles.length > 0 &&
					!hasVirtualFiles
			},
			{type: 'separator'},
			{
				icon: Copy,
				label:
					selected && selectedFiles.length > 1
						? `Copy ${selectedFiles.length} items`
						: 'Copy',
				onClick: () => {
					const targets = getTargetFiles();
					if (targets.length === 0) {
						console.warn('Cannot copy virtual files');
						return;
					}
					const sdPaths = targets.map((f) => f.sd_path);
					clipboard.copyFiles(sdPaths, currentPath);
				},
				keybindId: 'explorer.copy',
				condition: () => !hasVirtualFiles
			},
			{
				icon: Scissors,
				label:
					selected && selectedFiles.length > 1
						? `Cut ${selectedFiles.length} items`
						: 'Cut',
				onClick: () => {
					const targets = getTargetFiles();
					if (targets.length === 0) {
						console.warn('Cannot cut virtual files');
						return;
					}
					const sdPaths = targets.map((f) => f.sd_path);
					clipboard.cutFiles(sdPaths, currentPath);
				},
				keybindId: 'explorer.cut',
				condition: () => !hasVirtualFiles
			},
			{
				icon: Copy,
				label: 'Paste',
				onClick: () => {
					if (!clipboard.hasClipboard() || !currentPath) {
						console.log(
							'[Clipboard] Nothing to paste or no destination'
						);
						return;
					}

					const operation =
						clipboard.operation === 'cut' ? 'move' : 'copy';

					console.groupCollapsed(
						`[Clipboard] Pasting ${clipboard.files.length} file${clipboard.files.length === 1 ? '' : 's'} (${operation})`
					);
					console.log('Operation:', operation);
					console.log('Destination:', currentPath);
					console.log('Source files (SdPath objects):');
					clipboard.files.forEach((file, index) => {
						console.log(
							`  [${index}]:`,
							JSON.stringify(file, null, 2)
						);
					});
					console.groupEnd();

					openFileOperation({
						operation,
						sources: clipboard.files,
						destination: currentPath,
						onComplete: () => {
							// Clear clipboard after cut operation completes
							if (clipboard.operation === 'cut') {
								console.log(
									'[Clipboard] Operation completed, clearing clipboard'
								);
								clipboard.clearClipboard();
							} else {
								console.log(
									'[Clipboard] Copy operation completed'
								);
							}
						}
					});
				},
				keybindId: 'explorer.paste',
				condition: () => clipboard.hasClipboard()
			},
			{
				icon: TagIconComponent,
				label:
					selected && selectedFiles.length > 1
						? `Remove tag from ${selectedFiles.length} items`
						: 'Remove tag',
				onClick: async () => {
					if (mode.type !== 'tag') return;
					const targets = getTargetFiles();
					if (targets.length === 0) return;
					try {
						await unapplyTags.mutateAsync({
							targets: { type: 'File', ids: targets.map((f) => f.id) },
							tag_ids: [mode.tagId]
						});
					} catch (err) {
						console.error('Failed to remove tag:', err);
						toast.error(`Failed to remove tag: ${err}`);
					}
				},
				condition: () => mode.type === 'tag' && !hasVirtualFiles
			},
			{type: 'separator'},
			{
				icon: Trash,
				label:
					selected && selectedFiles.length > 1
						? `Delete ${selectedFiles.length} items`
						: 'Delete',
				onClick: async () => {
					const targets = getTargetFiles();
					await deleteFiles(targets, false);
				},
				keybind: '⌘⌫',
				variant: 'danger' as const,
				condition: () => !hasVirtualFiles
			}
		]
	});
}
