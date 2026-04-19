import {
	ArrowSquareOut,
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
import {useExplorer} from '../context';
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
	const platform = usePlatform();
	const refetchTagQueries = useRefetchTagQueries();

	const {deleteFiles} = useDeleteFiles();
	const unapplyTags = useLibraryMutation('tags.unapply', {
		onSuccess: refetchTagQueries
	});
	const createFolder = useLibraryMutation('files.createFolder');
	const regenerateThumbnail = useLibraryMutation(
		'media.thumbnail.regenerate'
	);
	const extractText = useLibraryMutation('media.ocr.extract');
	const transcribeAudio = useLibraryMutation('media.speech.transcribe');
	const generateThumbstrip = useLibraryMutation('media.thumbstrip.generate');
	const generateProxy = useLibraryMutation('media.proxy.generate');

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
			// Media Processing submenu
			{
				type: 'submenu',
				icon: Image,
				label: 'Image Processing',
				condition: () => !!file && getContentKind(file) === 'image',
				submenu: [
					{
						icon: Sparkle,
						label: 'Generate Blurhash',
						onClick: async () => {
							const targets = getTargetFiles();
							await forEachTarget(targets, (f) =>
								regenerateThumbnail.mutateAsync({
									entry_uuid: f.id,
									force: false
								})
							);
						},
						condition: () =>
							!!file && !file.image_media_data?.blurhash
					},
					{
						icon: Crop,
						label: 'Regenerate Thumbnail',
						onClick: async () => {
							const targets = getTargetFiles();
							await forEachTarget(targets, (f) =>
								regenerateThumbnail.mutateAsync({
									entry_uuid: f.id,
									force: true
								})
							);
						}
					},
					{
						icon: TextAa,
						label: 'Extract Text (OCR)',
						onClick: async () => {
							const targets = getTargetFiles();
							await forEachTarget(targets, (f) =>
								extractText.mutateAsync({
									entry_uuid: f.id,
									force: false
								})
							);
						},
						keybind: '⌘⇧T'
					}
				]
			},
			{
				type: 'submenu',
				icon: Video,
				label: 'Video Processing',
				condition: () => !!file && getContentKind(file) === 'video',
				submenu: [
					{
						icon: FilmStrip,
						label: 'Generate Thumbstrip',
						onClick: async () => {
							const targets = getTargetFiles();
							await forEachTarget(targets, (f) =>
								generateThumbstrip.mutateAsync({
									entry_uuid: f.id,
									force: false
								})
							);
						},
						condition: () =>
							!!file &&
							!file.sidecars?.some((s) => s.kind === 'thumbstrip')
					},
					{
						icon: Sparkle,
						label: 'Generate Blurhash',
						onClick: async () => {
							const targets = getTargetFiles();
							await forEachTarget(targets, (f) =>
								regenerateThumbnail.mutateAsync({
									entry_uuid: f.id,
									force: false
								})
							);
						},
						condition: () =>
							!!file && !file.video_media_data?.blurhash
					},
					{
						icon: Crop,
						label: 'Regenerate Thumbnail',
						onClick: async () => {
							const targets = getTargetFiles();
							await forEachTarget(targets, (f) =>
								regenerateThumbnail.mutateAsync({
									entry_uuid: f.id,
									force: true
								})
							);
						}
					},
					{
						icon: Waveform,
						label: 'Extract Subtitles',
						onClick: async () => {
							const targets = getTargetFiles();
							await forEachTarget(targets, (f) =>
								transcribeAudio.mutateAsync({
									entry_uuid: f.id
								})
							);
						}
					},
					{
						icon: FileVideo,
						label: 'Generate Proxy',
						onClick: async () => {
							const targets = getTargetFiles();
							await forEachTarget(targets, (f) =>
								generateProxy.mutateAsync({
									entry_uuid: f.id,
									force: false
								})
							);
						},
						keybind: '⌘⇧P'
					}
				]
			},
			{
				type: 'submenu',
				icon: Microphone,
				label: 'Audio Processing',
				condition: () => !!file && getContentKind(file) === 'audio',
				submenu: [
					{
						icon: TextAa,
						label: 'Transcribe Audio',
						onClick: async () => {
							const targets = getTargetFiles();
							await forEachTarget(targets, (f) =>
								transcribeAudio.mutateAsync({
									entry_uuid: f.id,
									model: 'whisper-base'
								})
							);
						},
						keybind: '⌘⇧T'
					}
				]
			},
			{
				type: 'submenu',
				icon: FileText,
				label: 'Document Processing',
				condition: () =>
					!!file &&
					file.kind === 'File' &&
					['pdf', 'doc', 'docx'].includes(file.extension || ''),
				submenu: [
					{
						icon: TextAa,
						label: 'Extract Text (OCR)',
						onClick: async () => {
							const targets = getTargetFiles();
							await forEachTarget(targets, (f) =>
								extractText.mutateAsync({
									entry_uuid: f.id,
									force: false
								})
							);
						},
						keybind: '⌘⇧T'
					},
					{
						icon: Crop,
						label: 'Regenerate Thumbnail',
						onClick: async () => {
							const targets = getTargetFiles();
							await forEachTarget(targets, (f) =>
								regenerateThumbnail.mutateAsync({
									entry_uuid: f.id,
									force: true
								})
							);
						}
					}
				]
			},
			// Batch operations submenu
			{
				type: 'submenu',
				icon: Stack,
				label: `Process ${selectedFiles.length} Items`,
				condition: () => selected && selectedFiles.length > 1,
				submenu: [
					{
						icon: Crop,
						label: 'Regenerate All Thumbnails',
						onClick: async () => {
							await forEachTarget(selectedFiles, (f) =>
								regenerateThumbnail.mutateAsync({
									entry_uuid: f.id,
									force: true
								})
							);
						}
					},
					{
						icon: Sparkle,
						label: 'Generate Blurhashes',
						onClick: async () => {
							await forEachTarget(selectedFiles, (f) =>
								regenerateThumbnail.mutateAsync({
									entry_uuid: f.id,
									force: false
								})
							);
						},
						keybind: '⌘⇧B'
					},
					{
						icon: TextAa,
						label: 'Extract Text (OCR)',
						onClick: async () => {
							await forEachTarget(selectedFiles, (f) =>
								extractText.mutateAsync({
									entry_uuid: f.id,
									force: false
								})
							);
						}
					}
				]
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
							entry_ids: targets.map((f) => f.id),
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
