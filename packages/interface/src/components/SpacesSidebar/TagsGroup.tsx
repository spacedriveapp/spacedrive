import { Tag as TagIcon, Plus, CaretRight } from '@phosphor-icons/react';
import { useState } from 'react';
import { useNavigate, useLocation } from 'react-router-dom';
import clsx from 'clsx';
import { useQueryClient } from '@tanstack/react-query';
import { useNormalizedQuery, useLibraryMutation } from '../../contexts/SpacedriveContext';
import type { Tag } from '@sd/ts-client';
import { GroupHeader } from './GroupHeader';
import { useExplorer } from '../../routes/explorer/context';

interface TagsGroupProps {
	isCollapsed: boolean;
	onToggle: () => void;
	sortableAttributes?: any;
	sortableListeners?: any;
}

interface TagItemProps {
	tag: Tag;
	depth?: number;
}

function TagItem({ tag, depth = 0 }: TagItemProps) {
	const navigate = useNavigate();
	const location = useLocation();
	const { loadPreferencesForSpaceItem } = useExplorer();
	const [isExpanded, setIsExpanded] = useState(false);
	const queryClient = useQueryClient();
	const deleteTag = useLibraryMutation('tags.delete', {
		onSuccess: () => {
			queryClient.refetchQueries({ queryKey: ["query:tags.search"], exact: false });
			queryClient.refetchQueries({ queryKey: ["query:files.by_tag"], exact: false });
		},
	});

	const children: Tag[] = [];
	const hasChildren = children.length > 0;
	const isActive = location.pathname === `/tag/${tag.id}`;

	const handleClick = () => {
		loadPreferencesForSpaceItem(`tag:${tag.id}`);
		navigate(`/tag/${tag.id}`);
	};

	const handleContextMenu = async (e: React.MouseEvent) => {
		e.preventDefault();
		e.stopPropagation();

		if (confirm(`Delete tag "${tag.canonical_name}"? This will remove it from all files.`)) {
			try {
				await deleteTag.mutateAsync({ tag_id: tag.id });
				if (isActive) {
					navigate('/');
				}
			} catch (err) {
				console.error('Failed to delete tag:', err);
				alert(`Failed to delete tag: ${err}`);
			}
		}
	};

	return (
		<div>
			<button
				onClick={handleClick}
				onContextMenu={handleContextMenu}
				className={clsx(
					'flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-sm font-medium transition-colors cursor-pointer',
					isActive
						? 'bg-sidebar-selected/30 text-sidebar-ink'
						: 'text-sidebar-ink-dull hover:bg-sidebar-box hover:text-sidebar-ink',
					tag.privacy_level === 'Archive' && 'opacity-50',
					tag.privacy_level === 'Hidden' && 'opacity-25'
				)}
				style={{ paddingLeft: `${8 + depth * 12}px` }}
			>
				{hasChildren && (
					<CaretRight
						size={10}
						weight="bold"
						className={clsx(
							'transition-transform flex-shrink-0',
							isExpanded && 'rotate-90'
						)}
						onClick={(e) => {
							e.stopPropagation();
							setIsExpanded(!isExpanded);
						}}
					/>
				)}

				{tag.icon ? (
					<TagIcon size={16} weight="bold" style={{ color: tag.color || '#3B82F6' }} />
				) : (
					<span
						className="size-2 rounded-full flex-shrink-0"
						style={{ backgroundColor: tag.color || '#3B82F6' }}
					/>
				)}

				<span className="flex-1 truncate text-left">{tag.canonical_name}</span>
			</button>

			{isExpanded &&
				children.map((child) => <TagItem key={child.id} tag={child} depth={depth + 1} />)}
		</div>
	);
}

export function TagsGroup({
	isCollapsed,
	onToggle,
	sortableAttributes,
	sortableListeners,
}: TagsGroupProps) {
	const navigate = useNavigate();
	const { loadPreferencesForSpaceItem } = useExplorer();
	const [isCreating, setIsCreating] = useState(false);
	const [newTagName, setNewTagName] = useState('');

	const createTag = useLibraryMutation('tags.create');

	const { data: tags = [], isLoading } = useNormalizedQuery({
		query: 'tags.search',
		input: { query: '' },
		resourceType: 'tag',
		// TODO: replace `any` with proper generated types when available
		select: (data: any) => data?.tags?.map((result: any) => result.tag || result).filter(Boolean) ?? []
	});

	const handleCreateTag = async () => {
		if (!newTagName.trim()) return;

		try {
			const result = await createTag.mutateAsync({
				canonical_name: newTagName.trim(),
				display_name: null,
				formal_name: null,
				abbreviation: null,
				aliases: [],
				namespace: null,
				tag_type: null,
				color: `#${Math.floor(Math.random() * 16777215).toString(16).padStart(6, '0')}`,
				icon: null,
				description: null,
				is_organizational_anchor: null,
				privacy_level: null,
				search_weight: null,
				attributes: null,
				apply_to: null
			});

			if (result?.tag_id) {
				loadPreferencesForSpaceItem(`tag:${result.tag_id}`);
				navigate(`/tag/${result.tag_id}`);
			}

			setNewTagName('');
			setIsCreating(false);
		} catch (err) {
			console.error('Failed to create tag:', err);
		}
	};

	return (
		<div>
			<GroupHeader
				label="Tags"
				isCollapsed={isCollapsed}
				onToggle={onToggle}
				sortableAttributes={sortableAttributes}
				sortableListeners={sortableListeners}
				rightComponent={
					tags.length > 0 && (
						<span className="ml-auto text-sidebar-ink-faint">{tags.length}</span>
					)
				}
			/>

			{!isCollapsed && (
				<div className="space-y-0.5">
					{isLoading ? (
						<div className="px-2 py-1 text-xs text-sidebar-ink-faint">Loading...</div>
					) : tags.length === 0 ? (
						<div className="px-2 py-1 text-xs text-sidebar-ink-faint">No tags yet</div>
					) : (
						tags.map((tag) => <TagItem key={tag.id} tag={tag} />)
					)}

					{isCreating ? (
						<div className="px-2 py-1.5">
							<input
								type="text"
								value={newTagName}
								onChange={(e) => setNewTagName(e.target.value)}
								onKeyDown={(e) => {
									if (e.key === 'Enter') {
										handleCreateTag();
									} else if (e.key === 'Escape') {
										setIsCreating(false);
										setNewTagName('');
									}
								}}
								onBlur={() => {
									if (!newTagName.trim()) {
										setIsCreating(false);
									}
								}}
								placeholder="Tag name..."
								autoFocus
								className="w-full px-2 py-1 text-xs rounded-md bg-sidebar-box border border-sidebar-line text-sidebar-ink placeholder:text-sidebar-ink-faint outline-none focus:border-accent"
							/>
						</div>
					) : (
						<button
							onClick={() => setIsCreating(true)}
							className="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-xs font-medium text-sidebar-ink-dull hover:bg-sidebar-box hover:text-sidebar-ink transition-colors cursor-pointer"
						>
							<Plus size={12} weight="bold" />
							<span>New Tag</span>
						</button>
					)}
				</div>
			)}
		</div>
	);
}
