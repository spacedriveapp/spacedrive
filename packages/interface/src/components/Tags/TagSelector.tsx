import { useState, useEffect } from 'react';
import { AnimatePresence, motion } from 'framer-motion';
import { MagnifyingGlass, Plus } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useNormalizedQuery, useLibraryMutation } from '../../context';
import type { Tag } from '@sd/ts-client';

interface TagSelectorProps {
	onSelect: (tag: Tag) => void;
	onClose?: () => void;
	contextTags?: Tag[];
	autoFocus?: boolean;
	className?: string;
}

/**
 * Dropdown menu for searching and selecting tags
 * Features fuzzy search, context-aware suggestions, and keyboard navigation
 */
export function TagSelector({
	onSelect,
	onClose,
	contextTags = [],
	autoFocus = true,
	className
}: TagSelectorProps) {
	const [query, setQuery] = useState('');
	const [selectedIndex, setSelectedIndex] = useState(0);

	const createTag = useLibraryMutation('tags.create');

	// Fetch all tags
	const { data: tagsData } = useNormalizedQuery({
		wireMethod: 'query:tags.list',
		input: null,
		resourceType: 'tag'
	});

	const allTags = tagsData?.tags ?? [];

	// Check if query matches an existing tag
	const exactMatch = allTags.find(
		tag => tag.canonical_name.toLowerCase() === query.toLowerCase()
	);

	// Filter tags based on search query
	const filteredTags = query.length > 0
		? allTags.filter(tag =>
			tag.canonical_name.toLowerCase().includes(query.toLowerCase()) ||
			tag.aliases?.some(alias => alias.toLowerCase().includes(query.toLowerCase())) ||
			tag.abbreviation?.toLowerCase().includes(query.toLowerCase())
		)
		: allTags;

	// Reset selected index when filtered tags change
	useEffect(() => {
		setSelectedIndex(0);
	}, [filteredTags.length]);

	// Keyboard navigation
	const handleKeyDown = async (e: React.KeyboardEvent) => {
		if (e.key === 'ArrowDown') {
			e.preventDefault();
			setSelectedIndex(prev => Math.min(prev + 1, filteredTags.length - 1));
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			setSelectedIndex(prev => Math.max(prev - 1, 0));
		} else if (e.key === 'Enter') {
			e.preventDefault();
			// If there's a match, select it
			if (filteredTags[selectedIndex]) {
				handleSelect(filteredTags[selectedIndex]!);
			}
			// If there's text but no match, create new tag
			else if (query.trim().length > 0 && !exactMatch) {
				await handleCreateTag();
			}
		} else if (e.key === 'Escape') {
			e.preventDefault();
			onClose?.();
		}
	};

	const handleSelect = (tag: Tag) => {
		onSelect(tag);
		setQuery('');
		onClose?.();
	};

	const handleCreateTag = async () => {
		if (!query.trim()) return;

		try {
			const newTag = await createTag.mutateAsync({
				canonical_name: query.trim(),
				color: `#${Math.floor(Math.random() * 16777215).toString(16).padStart(6, '0')}`, // Random color
			});

			// Select the newly created tag
			if (newTag?.tag) {
				onSelect(newTag.tag);
				setQuery('');
				onClose?.();
			}
		} catch (err) {
			console.error('Failed to create tag:', err);
		}
	};

	return (
		<div className={clsx('flex flex-col bg-menu border border-menu-line rounded-lg shadow-lg overflow-hidden', className)}>
			{/* Search Input */}
			<div className="flex items-center gap-2 px-3 py-2 border-b border-menu-line">
				<MagnifyingGlass size={16} className="text-menu-ink-dull flex-shrink-0" />
				<input
					type="text"
					value={query}
					onChange={(e) => setQuery(e.target.value)}
					onKeyDown={handleKeyDown}
					placeholder="Search tags..."
					autoFocus={autoFocus}
					className="flex-1 bg-transparent text-sm text-menu-ink placeholder:text-menu-ink-faint outline-none"
				/>
			</div>

			{/* Results */}
			<div className="max-h-64 overflow-y-auto">
				{/* Create new tag option */}
				{query.trim().length > 0 && !exactMatch && (
					<button
						onClick={handleCreateTag}
						onMouseEnter={() => setSelectedIndex(-1)}
						className={clsx(
							'flex items-center gap-2 w-full px-3 py-2 text-sm transition-colors border-b border-menu-line',
							selectedIndex === -1
								? 'bg-menu-hover text-menu-ink'
								: 'text-menu-ink-dull hover:bg-menu-hover hover:text-menu-ink'
						)}
					>
						<Plus size={16} weight="bold" className="flex-shrink-0" />
						<span className="flex-1 text-left">
							Create tag "<strong>{query}</strong>"
						</span>
						<kbd className="text-xs text-menu-ink-faint px-1.5 py-0.5 rounded bg-menu-line">
							↵
						</kbd>
					</button>
				)}

				{filteredTags.length === 0 && !query.trim() ? (
					<div className="px-3 py-4 text-sm text-menu-ink-dull text-center">
						No tags yet
					</div>
				) : filteredTags.length === 0 && query.trim() ? null : (
					filteredTags.map((tag, index) => (
						<button
							key={tag.id}
							onClick={() => handleSelect(tag)}
							onMouseEnter={() => setSelectedIndex(index)}
							className={clsx(
								'flex items-center gap-2 w-full px-3 py-2 text-sm transition-colors',
								index === selectedIndex
									? 'bg-menu-hover text-menu-ink'
									: 'text-menu-ink-dull hover:bg-menu-hover hover:text-menu-ink'
							)}
						>
							{/* Color dot */}
							<span
								className="size-2 rounded-full flex-shrink-0"
								style={{ backgroundColor: tag.color || '#3B82F6' }}
							/>

							{/* Tag name */}
							<span className="flex-1 text-left truncate">{tag.canonical_name}</span>

							{/* Namespace badge */}
							{tag.namespace && (
								<span className="text-xs text-menu-ink-faint px-1.5 py-0.5 rounded bg-menu-line">
									{tag.namespace}
								</span>
							)}
						</button>
					))
				)}
			</div>
		</div>
	);
}

interface TagSelectorButtonProps {
	onSelect: (tag: Tag) => void;
	trigger: React.ReactNode;
	contextTags?: Tag[];
}

/**
 * Wrapper component that shows TagSelector in a dropdown when trigger is clicked
 */
export function TagSelectorButton({ onSelect, trigger, contextTags }: TagSelectorButtonProps) {
	const [isOpen, setIsOpen] = useState(false);

	return (
		<div className="relative">
			<div onClick={() => setIsOpen(!isOpen)}>
				{trigger}
			</div>

			<AnimatePresence>
				{isOpen && (
					<>
						{/* Backdrop */}
						<div
							className="fixed inset-0 z-40"
							onClick={() => setIsOpen(false)}
						/>

						{/* Dropdown */}
						<motion.div
							initial={{ opacity: 0, y: -8 }}
							animate={{ opacity: 1, y: 0 }}
							exit={{ opacity: 0, y: -8 }}
							transition={{ duration: 0.15 }}
							className="absolute top-full left-0 mt-1 w-64 z-50"
						>
							<TagSelector
								onSelect={(tag) => {
									onSelect(tag);
									setIsOpen(false);
								}}
								onClose={() => setIsOpen(false)}
								contextTags={contextTags}
							/>
						</motion.div>
					</>
				)}
			</AnimatePresence>
		</div>
	);
}
