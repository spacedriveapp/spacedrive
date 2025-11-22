import { useParams } from 'react-router-dom';
import { CaretRight, Funnel } from '@phosphor-icons/react';
import { Fragment } from 'react';
import { useNormalizedQuery } from '../../context';
import { ExplorerView } from '../../components/explorer';
import type { Tag } from '@sd/ts-client';

/**
 * Tag Explorer View
 * Shows all files tagged with a specific tag, with hierarchy awareness and filtering
 */
export function TagView() {
	const { tagId } = useParams<{ tagId: string }>();

	// Fetch the tag details
	const { data: tagData, isLoading: tagLoading } = useNormalizedQuery({
		wireMethod: 'query:tags.by_id',
		input: { tag_id: tagId },
		resourceType: 'tag',
		resourceId: tagId,
		enabled: !!tagId
	});

	// Fetch tag ancestors for breadcrumb
	const { data: ancestorsData } = useNormalizedQuery({
		wireMethod: 'query:tags.ancestors',
		input: { tag_id: tagId },
		resourceType: 'tag',
		resourceId: tagId,
		enabled: !!tagId
	});

	// Fetch tag children for quick filters
	const { data: childrenData } = useNormalizedQuery({
		wireMethod: 'query:tags.children',
		input: { tag_id: tagId },
		resourceType: 'tag',
		resourceId: tagId,
		enabled: !!tagId
	});

	// Fetch related tags for suggestions
	const { data: relatedData } = useNormalizedQuery({
		wireMethod: 'query:tags.related',
		input: { tag_id: tagId },
		resourceType: 'tag',
		resourceId: tagId,
		enabled: !!tagId
	});

	// Fetch files with this tag
	const { data: filesData, isLoading: filesLoading } = useNormalizedQuery({
		wireMethod: 'query:files.by_tag',
		input: {
			tag_id: tagId,
			include_children: false, // TODO: Make this toggleable
			min_confidence: 0.0
		},
		resourceType: 'file',
		enabled: !!tagId
	});

	const tag = tagData?.tag;
	const ancestors = ancestorsData?.ancestors ?? [];
	const children = childrenData?.children ?? [];
	const related = relatedData?.related ?? [];
	const files = filesData?.files ?? [];

	if (tagLoading) {
		return (
			<div className="flex items-center justify-center h-full">
				<span className="text-ink-dull">Loading tag...</span>
			</div>
		);
	}

	if (!tag) {
		return (
			<div className="flex items-center justify-center h-full">
				<span className="text-ink-dull">Tag not found</span>
			</div>
		);
	}

	return (
		<div className="flex h-full">
			{/* Main Content */}
			<div className="flex-1 flex flex-col">
				{/* Header */}
				<div className="px-4 py-3 border-b border-app-line space-y-3">
					{/* Breadcrumb */}
					<div className="flex items-center gap-2 text-sm">
						{ancestors.map((ancestor, i) => (
							<Fragment key={ancestor.id}>
								<button
									onClick={() => window.location.href = `/tag/${ancestor.id}`}
									className="text-ink-dull hover:text-ink font-medium transition-colors"
								>
									{ancestor.canonical_name}
								</button>
								<CaretRight size={12} className="text-ink-faint" />
							</Fragment>
						))}
						<div className="flex items-center gap-2">
							{tag.icon ? (
								<span style={{ color: tag.color || '#3B82F6' }}>
									{/* TODO: Render icon */}
								</span>
							) : (
								<span
									className="size-3 rounded-full"
									style={{ backgroundColor: tag.color || '#3B82F6' }}
								/>
							)}
							<span className="text-ink font-semibold">{tag.canonical_name}</span>
						</div>
					</div>

					{/* Options Row */}
					<div className="flex items-center justify-between">
						<div className="flex items-center gap-2">
							{/* TODO: Add filters button */}
							<button className="flex items-center gap-2 px-3 py-1.5 rounded-md bg-app-box border border-app-line text-sm hover:bg-app-hover transition-colors">
								<Funnel size={14} />
								<span>Filters</span>
							</button>
						</div>

						{/* File Count */}
						<span className="text-sm text-ink-dull">
							{filesLoading ? 'Loading...' : `${files.length} ${files.length === 1 ? 'file' : 'files'}`}
						</span>
					</div>

					{/* Child Tag Quick Filters */}
					{children.length > 0 && (
						<div className="flex items-center gap-2 flex-wrap">
							<span className="text-xs font-semibold text-ink-dull">Children:</span>
							{children.map((child) => (
								<button
									key={child.id}
									onClick={() => window.location.href = `/tag/${child.id}`}
									className="inline-flex items-center gap-1.5 px-2 py-1 rounded-md bg-app-box hover:bg-app-hover border border-app-line text-xs font-medium transition-colors"
									style={{ color: child.color || '#3B82F6' }}
								>
									<span
										className="size-1.5 rounded-full"
										style={{ backgroundColor: child.color || '#3B82F6' }}
									/>
									{child.canonical_name}
								</button>
							))}
						</div>
					)}
				</div>

				{/* Explorer View */}
				<div className="flex-1 overflow-auto">
					{filesLoading ? (
						<div className="flex items-center justify-center h-full">
							<span className="text-ink-dull">Loading files...</span>
						</div>
					) : files.length === 0 ? (
						<div className="flex flex-col items-center justify-center h-full gap-2">
							<span className="text-ink-dull">No files with this tag</span>
							<span className="text-xs text-ink-faint">Files will appear here when you tag them</span>
						</div>
					) : (
						<ExplorerView />
					)}
				</div>
			</div>

			{/* Sidebar: Related Tags */}
			{related.length > 0 && (
				<aside className="w-64 border-l border-app-line p-4 space-y-4 overflow-y-auto">
					<div>
						<h4 className="text-sm font-semibold text-ink-dull mb-2">Related Tags</h4>
						<div className="space-y-1">
							{related.map((relatedTag) => (
								<button
									key={relatedTag.id}
									onClick={() => window.location.href = `/tag/${relatedTag.id}`}
									className="flex items-center justify-between w-full px-2 py-1.5 rounded-md hover:bg-app-hover text-sm transition-colors"
								>
									<div className="flex items-center gap-2">
										<span
											className="size-2 rounded-full"
											style={{ backgroundColor: relatedTag.color || '#3B82F6' }}
										/>
										<span className="text-ink">{relatedTag.canonical_name}</span>
									</div>
									{relatedTag.co_occurrence_count && (
										<span className="text-xs text-ink-faint">
											{relatedTag.co_occurrence_count}
										</span>
									)}
								</button>
							))}
						</div>
					</div>
				</aside>
			)}
		</div>
	);
}
