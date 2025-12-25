import { useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Tag as TagIcon, X } from '@phosphor-icons/react';
import clsx from 'clsx';
import { Button } from '@sd/ui';
import { useNormalizedQuery, useLibraryMutation } from '../../context';
import { useSelection } from './SelectionContext';
import { useKeybind } from '../../hooks/useKeybind';
import type { Tag } from '@sd/ts-client';

interface TagAssignmentModeProps {
	isActive: boolean;
	onExit: () => void;
}

/**
 * Tag Assignment Mode - Quick keyboard-driven tagging
 *
 * Features:
 * - Toggle tags with number keys (1-9, 0)
 * - Switch palettes with Cmd+Shift+[1-9]
 * - Visual feedback for applied tags
 * - Works on selected files
 */
export function TagAssignmentMode({ isActive, onExit }: TagAssignmentModeProps) {
	const { selectedFiles } = useSelection();
	const [currentPaletteIndex, setCurrentPaletteIndex] = useState(0);

	const applyTag = useLibraryMutation('tags.apply');

	// Fetch all tags (for now, we'll use the first 10 as the default palette)
	// TODO: Implement user-defined palettes
	const { data: tagsData } = useNormalizedQuery<
		{ query: string },
		{ tags: Array<{ tag: Tag } | Tag> }
	>({
		wireMethod: 'query:tags.search',
		input: { query: '' },
		resourceType: 'tag'
	});

	// Extract tags from search results
	// Handle both wrapped format ({ tag, relevance }) from initial query
	// and raw Tag objects from real-time ResourceChanged events
	const allTags = tagsData?.tags?.map((result) =>
		'tag' in result ? result.tag : result
	) ?? [];
	const paletteTags = allTags.slice(0, 10) as Tag[];

	// Keyboard shortcuts using keybind registry
	useKeybind('explorer.exitTagMode', onExit, { enabled: isActive });
	useKeybind('explorer.toggleTag1', () => handleToggleTag(0), { enabled: isActive });
	useKeybind('explorer.toggleTag2', () => handleToggleTag(1), { enabled: isActive });
	useKeybind('explorer.toggleTag3', () => handleToggleTag(2), { enabled: isActive });
	useKeybind('explorer.toggleTag4', () => handleToggleTag(3), { enabled: isActive });
	useKeybind('explorer.toggleTag5', () => handleToggleTag(4), { enabled: isActive });
	useKeybind('explorer.toggleTag6', () => handleToggleTag(5), { enabled: isActive });
	useKeybind('explorer.toggleTag7', () => handleToggleTag(6), { enabled: isActive });
	useKeybind('explorer.toggleTag8', () => handleToggleTag(7), { enabled: isActive });
	useKeybind('explorer.toggleTag9', () => handleToggleTag(8), { enabled: isActive });
	useKeybind('explorer.toggleTag10', () => handleToggleTag(9), { enabled: isActive });

	const handleToggleTag = async (index: number) => {
		const tag = paletteTags[index];
		if (!tag || selectedFiles.length === 0) return;

		// Get content IDs from selected files (filter out files without content identity)
		const contentIds = selectedFiles
			.map(f => f.content_identity?.uuid)
			.filter((id): id is string => id != null);

		if (contentIds.length === 0) return;

		try {
			await applyTag.mutateAsync({
				targets: { type: 'Content', ids: contentIds },
				tag_ids: [tag.id]
			});
		} catch (err) {
			console.error('Failed to apply tag:', err);
		}
	};

	// Check if a tag is active (all selected files have it)
	const isTagActive = (tag: Tag) => {
		if (selectedFiles.length === 0) return false;
		return selectedFiles.every(file =>
			file.tags?.some(t => t.id === tag.id)
		);
	};

	if (!isActive) return null;

	return (
		<AnimatePresence>
			<motion.div
				initial={{ y: 100, opacity: 0 }}
				animate={{ y: 0, opacity: 1 }}
				exit={{ y: 100, opacity: 0 }}
				transition={{ duration: 0.2, ease: [0.25, 1, 0.5, 1] }}
				className="absolute bottom-2 left-1 right-1 z-50"
			>
				<div className="bg-sidebar/80 backdrop-blur-xl border border-sidebar-line/50 rounded-xl px-4 py-3 shadow-lg">
					<div className="flex items-center gap-3">
						{/* Mode Label */}
						<div className="flex items-center gap-2">
							<TagIcon size={16} weight="bold" className="text-accent" />
							<span className="text-sm font-semibold text-sidebar-ink">Tag Mode</span>
							{selectedFiles.length > 0 && (
								<span className="text-xs text-sidebar-inkDull">
									{selectedFiles.length} {selectedFiles.length === 1 ? 'item' : 'items'}
								</span>
							)}
						</div>

						{/* Palette Tags */}
						<div className="flex gap-1.5 flex-1">
							{paletteTags.map((tag, index) => {
								const active = isTagActive(tag);
								const number = index === 9 ? 0 : index + 1;

								return (
									<button
										key={tag.id}
										onClick={() => handleToggleTag(index)}
										className={clsx(
											'inline-flex items-center gap-2 rounded-md font-medium px-2.5 py-1 text-sm transition-all',
											active
												? 'shadow-md scale-105'
												: 'hover:scale-105'
										)}
										style={{
											backgroundColor: active ? `${tag.color || '#3B82F6'}40` : `${tag.color || '#3B82F6'}20`,
											color: tag.color || '#3B82F6'
										}}
									>
										{/* Keyboard Number */}
										<kbd className="px-1 py-0.5 text-[10px] font-bold rounded bg-black/20 min-w-[16px] text-center">
											{number}
										</kbd>

										{/* Tag Name */}
										<span className="truncate max-w-[120px]">{tag.canonical_name}</span>

										{/* Active Checkmark */}
										{active && (
											<span className="text-xs">✓</span>
										)}
									</button>
								);
							})}
						</div>

						{/* Exit Button */}
						<Button size="sm" variant="accent" onClick={onExit}>
							Done
						</Button>
					</div>

					{/* Help Text */}
					{selectedFiles.length === 0 && (
						<div className="mt-2 text-xs text-sidebar-inkFaint text-center">
							Select files to start tagging • Press 1-9/0 to toggle tags • Esc to exit
						</div>
					)}
				</div>
			</motion.div>
		</AnimatePresence>
	);
}
