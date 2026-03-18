import {CaretRight, File, Folder, Funnel} from '@phosphor-icons/react';
import type {SdPath} from '@sd/ts-client';
import {Fragment, useCallback} from 'react';
import {useNavigate, useParams} from 'react-router-dom';
import {useNormalizedQuery} from '../../contexts/SpacedriveContext';

interface TaggedFile {
	id: string;
	name: string;
	extension: string | null;
	size: number;
	kind: number;
	modified_at: string;
	sd_path: SdPath;
}

function formatBytes(bytes: number): string {
	if (bytes === 0) return '0 B';
	const k = 1024;
	const sizes = ['B', 'KB', 'MB', 'GB'];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

function getParentPath(sdPath: SdPath): SdPath | null {
	if (!('Physical' in sdPath)) return null;
	const {device_slug, path} = sdPath.Physical;
	const lastSep = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
	if (lastSep <= 0) return null;
	return {Physical: {device_slug, path: path.substring(0, lastSep)}};
}

/**
 * Tag Explorer View
 * Shows all files tagged with a specific tag, with hierarchy awareness and filtering
 */
export function TagView() {
	const {tagId} = useParams<{tagId: string}>();
	const navigate = useNavigate();

	const handleFileDoubleClick = useCallback(
		(file: TaggedFile) => {
			// For directories, navigate into them; for files, navigate to parent folder
			const targetPath =
				file.kind === 1 ? file.sd_path : getParentPath(file.sd_path);
			if (!targetPath) return;
			const encoded = encodeURIComponent(JSON.stringify(targetPath));
			navigate(`/explorer?path=${encoded}`);
		},
		[navigate]
	);

	// Fetch the tag details
	const {data: tagData, isLoading: tagLoading} = useNormalizedQuery({
		query: 'tags.by_id',
		input: {tag_id: tagId},
		resourceType: 'tag',
		resourceId: tagId,
		enabled: !!tagId
	});

	// Fetch tag ancestors for breadcrumb
	const {data: ancestorsData} = useNormalizedQuery({
		query: 'tags.ancestors',
		input: {tag_id: tagId},
		resourceType: 'tag',
		resourceId: tagId,
		enabled: !!tagId
	});

	// Fetch tag children for quick filters
	const {data: childrenData} = useNormalizedQuery({
		query: 'tags.children',
		input: {tag_id: tagId},
		resourceType: 'tag',
		resourceId: tagId,
		enabled: !!tagId
	});

	// Fetch related tags for suggestions
	const {data: relatedData} = useNormalizedQuery({
		query: 'tags.related',
		input: {tag_id: tagId},
		resourceType: 'tag',
		resourceId: tagId,
		enabled: !!tagId
	});

	// Fetch files with this tag
	const {data: filesData, isLoading: filesLoading} = useNormalizedQuery({
		query: 'files.by_tag',
		input: {
			tag_id: tagId,
			include_children: false,
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
			<div className="flex h-full items-center justify-center">
				<span className="text-ink-dull">Loading tag...</span>
			</div>
		);
	}

	if (!tag) {
		return (
			<div className="flex h-full items-center justify-center">
				<span className="text-ink-dull">Tag not found</span>
			</div>
		);
	}

	return (
		<div className="flex h-full">
			{/* Main Content */}
			<div className="flex flex-1 flex-col">
				{/* Header */}
				<div className="border-app-line space-y-3 border-b px-4 py-3">
					{/* Breadcrumb */}
					<div className="flex items-center gap-2 text-sm">
						{ancestors.map((ancestor, i) => (
							<Fragment key={ancestor.id}>
								<button
									onClick={() =>
										navigate(`/tag/${ancestor.id}`)
									}
									className="text-ink-dull hover:text-ink font-medium transition-colors"
								>
									{ancestor.canonical_name}
								</button>
								<CaretRight
									size={12}
									className="text-ink-faint"
								/>
							</Fragment>
						))}
						<div className="flex items-center gap-2">
							{tag.icon ? (
								<span style={{color: tag.color || '#3B82F6'}}>
									{/* TODO: Render icon */}
								</span>
							) : (
								<span
									className="size-3 rounded-full"
									style={{
										backgroundColor: tag.color || '#3B82F6'
									}}
								/>
							)}
							<span className="text-ink font-semibold">
								{tag.canonical_name}
							</span>
						</div>
					</div>

					{/* Options Row */}
					<div className="flex items-center justify-between">
						<div className="flex items-center gap-2">
							{/* TODO: Add filters button */}
							<button className="bg-app-box border-app-line hover:bg-app-hover flex items-center gap-2 rounded-md border px-3 py-1.5 text-sm transition-colors">
								<Funnel size={14} />
								<span>Filters</span>
							</button>
						</div>

						{/* File Count */}
						<span className="text-ink-dull text-sm">
							{filesLoading
								? 'Loading...'
								: `${files.length} ${files.length === 1 ? 'file' : 'files'}`}
						</span>
					</div>

					{/* Child Tag Quick Filters */}
					{children.length > 0 && (
						<div className="flex flex-wrap items-center gap-2">
							<span className="text-ink-dull text-xs font-semibold">
								Children:
							</span>
							{children.map((child) => (
								<button
									key={child.id}
									onClick={() =>
										navigate(`/tag/${child.id}`)
									}
									className="bg-app-box hover:bg-app-hover border-app-line inline-flex items-center gap-1.5 rounded-md border px-2 py-1 text-xs font-medium transition-colors"
									style={{color: child.color || '#3B82F6'}}
								>
									<span
										className="size-1.5 rounded-full"
										style={{
											backgroundColor:
												child.color || '#3B82F6'
										}}
									/>
									{child.canonical_name}
								</button>
							))}
						</div>
					)}
				</div>

				{/* Tagged Files */}
				<div className="flex-1 overflow-auto">
					{filesLoading ? (
						<div className="flex h-full items-center justify-center">
							<span className="text-ink-dull">
								Loading files...
							</span>
						</div>
					) : files.length === 0 ? (
						<div className="flex h-full flex-col items-center justify-center gap-2">
							<span className="text-ink-dull">
								No files with this tag
							</span>
							<span className="text-ink-faint text-xs">
								Files will appear here when you tag them
							</span>
						</div>
					) : (
						<div className="p-2 space-y-0.5">
							{(files as TaggedFile[]).map((file) => (
								<div
									key={file.id}
									className="flex items-center gap-3 rounded-md px-3 py-2 hover:bg-app-hover transition-colors cursor-pointer"
									onDoubleClick={() =>
										handleFileDoubleClick(file)
									}
								>
									{file.kind === 1 ? (
										<Folder size={16} className="text-ink-faint flex-shrink-0" />
									) : (
										<File size={16} className="text-ink-faint flex-shrink-0" />
									)}
									<span className="flex-1 truncate text-sm text-ink">
										{file.name}
										{file.extension && `.${file.extension}`}
									</span>
									{file.extension && (
										<span className="text-xs text-ink-faint uppercase">
											{file.extension}
										</span>
									)}
									<span className="text-xs text-ink-faint tabular-nums">
										{formatBytes(file.size)}
									</span>
								</div>
							))}
						</div>
					)}
				</div>
			</div>

			{/* Sidebar: Related Tags */}
			{related.length > 0 && (
				<aside className="border-app-line w-64 space-y-4 overflow-y-auto border-l p-4">
					<div>
						<h4 className="text-ink-dull mb-2 text-sm font-semibold">
							Related Tags
						</h4>
						<div className="space-y-1">
							{related.map((relatedTag) => (
								<button
									key={relatedTag.id}
									onClick={() =>
										navigate(`/tag/${relatedTag.id}`)
									}
									className="hover:bg-app-hover flex w-full items-center justify-between rounded-md px-2 py-1.5 text-sm transition-colors"
								>
									<div className="flex items-center gap-2">
										<span
											className="size-2 rounded-full"
											style={{
												backgroundColor:
													relatedTag.color ||
													'#3B82F6'
											}}
										/>
										<span className="text-ink">
											{relatedTag.canonical_name}
										</span>
									</div>
									{relatedTag.co_occurrence_count && (
										<span className="text-ink-faint text-xs">
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
