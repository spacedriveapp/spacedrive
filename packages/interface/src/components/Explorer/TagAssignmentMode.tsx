import { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Tag as TagIcon, X } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useNormalizedQuery, useLibraryMutation } from '../../context';
import { useSelection } from './SelectionContext';
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
	const { data: tagsData } = useNormalizedQuery({
		wireMethod: 'query:tags.search',
		input: { query: '' },
		resourceType: 'tag'
	});

	// Extract tags from search results (tags is an array of { tag, relevance, ... })
	const allTags = tagsData?.tags?.map((result: any) => result.tag) ?? [];
	const paletteTags = allTags.slice(0, 10); // First 10 tags for now

	// Keyboard shortcuts
	useEffect(() => {
		if (!isActive) return;

		const handleKeyDown = (e: KeyboardEvent) => {
			// Exit on Escape
			if (e.key === 'Escape') {
				e.preventDefault();
				onExit();
				return;
			}

			// Number keys 1-9, 0
			if (e.key >= '1' && e.key <= '9') {
				e.preventDefault();
				const index = parseInt(e.key) - 1;
				handleToggleTag(index);
			} else if (e.key === '0') {
				e.preventDefault();
				handleToggleTag(9);
			}

			// TODO: Palette switching with Cmd+Shift+[1-9]
		};

		window.addEventListener('keydown', handleKeyDown);
		return () => window.removeEventListener('keydown', handleKeyDown);
	}, [isActive, selectedFiles, paletteTags]);

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
				initial={{ y: -100, opacity: 0 }}
				animate={{ y: 0, opacity: 1 }}
				exit={{ y: -100, opacity: 0 }}
				transition={{ duration: 0.2, ease: [0.25, 1, 0.5, 1] }}
				className="fixed top-[52px] left-0 right-0 bg-app-box/95 backdrop-blur-lg border-b border-app-line z-50 px-4 py-2 shadow-lg"
			>
				<div className="flex items-center gap-3">
					{/* Mode Label */}
					<div className="flex items-center gap-2">
						<TagIcon size={16} weight="bold" className="text-accent" />
						<span className="text-sm font-semibold text-ink">Tag Mode</span>
						{selectedFiles.length > 0 && (
							<span className="text-xs text-ink-dull">
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
										'inline-flex items-center gap-2 rounded-lg font-medium px-3 py-1.5 text-sm transition-all',
										active
											? 'ring-2 ring-accent shadow-md scale-105'
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
					<button
						onClick={onExit}
						className="flex items-center gap-2 px-3 py-1.5 text-sm font-medium rounded-md bg-accent hover:bg-accent-deep text-white transition-colors"
					>
						Done
					</button>
				</div>

				{/* Help Text */}
				{selectedFiles.length === 0 && (
					<div className="mt-2 text-xs text-ink-faint text-center">
						Select files to start tagging • Press 1-9/0 to toggle tags • Esc to exit
					</div>
				)}
			</motion.div>
		</AnimatePresence>
	);
}
