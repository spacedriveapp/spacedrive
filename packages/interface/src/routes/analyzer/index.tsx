import {ArrowLeft, HardDrive, Plugs} from '@phosphor-icons/react';
import type {IndexSourceInfo, SizeTreeOutput} from '@sd/ts-client';
import {useState} from 'react';
import {useNavigate, useSearchParams} from 'react-router-dom';
import {useCoreQuery} from '../../contexts/SpacedriveContext';
import {formatBytes} from '../overview/DevicePanel';
import {Sunburst} from './Sunburst';

/**
 * The storage analyzer. `/analyzer` renders every indexed source as a small
 * sunburst in a scrolling grid; clicking a drive loads it in full fidelity.
 * The analyzer is a view over the index — sizes come from arena rollups via
 * files.size_tree, so drives answer even while unplugged.
 */
export function AnalyzerView() {
	const [params, setParams] = useSearchParams();
	const path = params.get('path');

	if (path) {
		return (
			<AnalyzerDetail
				path={path}
				onNavigate={(p) => setParams(p ? {path: p} : {})}
			/>
		);
	}
	return <AnalyzerGrid onOpen={(p) => setParams({path: p})} />;
}

function AnalyzerGrid({onOpen}: {onOpen: (path: string) => void}) {
	const navigate = useNavigate();
	const {data: status} = useCoreQuery({
		type: 'core.index_status',
		input: {}
	});
	const sources = (status?.sources ?? []) as IndexSourceInfo[];

	return (
		<div className="h-full overflow-y-auto p-8 pt-[52px]">
			<div className="flex items-center gap-3">
				<button
					type="button"
					onClick={() => navigate('/sources')}
					className="text-ink-dull hover:bg-app-hover hover:text-ink rounded-md px-2 py-1 text-sm"
				>
					Storage
				</button>
				<span className="text-ink-faint">/</span>
				<h1 className="text-ink text-xl font-semibold">Usage</h1>
			</div>
			<p className="text-ink-faint mt-1 text-sm">
				See what occupies each indexed source, including sources that
				are offline.
			</p>

			{sources.length === 0 && (
				<div className="text-ink-faint mt-16 text-center text-sm">
					Nothing indexed yet. Track storage and it appears here.
				</div>
			)}

			<div className="mt-8 grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-4">
				{sources.map((source) => (
					<SourceCard
						key={source.id}
						source={source}
						onOpen={() => onOpen(source.root)}
					/>
				))}
			</div>
		</div>
	);
}

function SourceCard({
	source,
	onOpen
}: {
	source: IndexSourceInfo;
	onOpen: () => void;
}) {
	const {data} = useCoreQuery({
		type: 'files.size_tree',
		input: {path: source.root, depth: 2, top: 8}
	});
	const tree = data as SizeTreeOutput | undefined;
	const name = source.root.split('/').filter(Boolean).pop() ?? source.root;

	return (
		<button
			onClick={onOpen}
			className={`border-app-line/60 bg-app-box/40 hover:border-app-line hover:bg-app-box group flex flex-col items-center rounded-lg border p-4 text-left transition-colors ${
				source.attached ? '' : 'opacity-70'
			}`}
		>
			<div className="relative">
				{tree ? (
					<Sunburst root={tree.root} size={170} />
				) : (
					<div className="flex size-[170px] items-center justify-center">
						<HardDrive className="text-ink-faint/40 size-10" />
					</div>
				)}
			</div>
			<div className="mt-2 flex w-full items-center gap-1.5">
				<span className="text-ink min-w-0 flex-1 truncate text-sm font-medium">
					{name}
				</span>
				{!source.attached && (
					<span title="Not plugged in">
						<Plugs className="text-ink-faint size-3.5 shrink-0" />
					</span>
				)}
			</div>
			<div className="text-ink-faint w-full text-xs">
				{source.total_bytes != null
					? formatBytes(source.total_bytes)
					: '—'}
				{source.entry_count != null &&
					` · ${source.entry_count.toLocaleString()} items`}
			</div>
		</button>
	);
}

function AnalyzerDetail({
	path,
	onNavigate
}: {
	path: string;
	onNavigate: (path: string | null) => void;
}) {
	const navigate = useNavigate();
	const [hovered, setHovered] = useState<{
		label: string;
		size: number;
	} | null>(null);

	const {data} = useCoreQuery({
		type: 'files.size_tree',
		input: {path, depth: 3, top: 16}
	});
	const tree = data as SizeTreeOutput | undefined;
	const root = tree?.root;

	const parent = path.split('/').slice(0, -1).join('/');

	return (
		<div className="flex h-full flex-col overflow-hidden p-8 pt-[52px]">
			<div className="flex items-center gap-3">
				<button
					onClick={() => onNavigate(null)}
					className="text-ink-dull hover:bg-app-hover hover:text-ink flex items-center gap-1 rounded-md px-2 py-1 text-sm"
				>
					<ArrowLeft className="size-4" /> All drives
				</button>
				{parent && (
					<button
						onClick={() => onNavigate(parent)}
						className="text-ink-dull hover:bg-app-hover hover:text-ink rounded-md px-2 py-1 text-sm"
					>
						Up
					</button>
				)}
				<span className="text-ink-faint min-w-0 flex-1 truncate text-sm">
					{path}
				</span>
			</div>

			<div className="flex min-h-0 flex-1 items-center justify-center gap-10">
				<div className="relative">
					{root && (
						<Sunburst
							root={root}
							size={440}
							onSelect={(p, isDir) => isDir && onNavigate(p)}
							onHover={setHovered}
						/>
					)}
					<div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center text-center">
						<span className="text-ink max-w-[120px] truncate text-sm font-semibold">
							{hovered?.label ?? root?.name}
						</span>
						<span className="text-ink-faint text-xs">
							{formatBytes(hovered?.size ?? root?.size ?? 0)}
						</span>
					</div>
				</div>

				{root && (
					<div className="max-h-[440px] w-72 overflow-y-auto">
						{root.children.map((child) => (
							<button
								key={child.path}
								onClick={() =>
									child.is_dir
										? onNavigate(child.path)
										: navigate(
												`/explorer?path=${encodeURIComponent(
													JSON.stringify({
														Physical: {
															device_slug:
																'local',
															path: parent
																? child.path
																		.split(
																			'/'
																		)
																		.slice(
																			0,
																			-1
																		)
																		.join(
																			'/'
																		)
																: child.path
														}
													})
												)}`
											)
								}
								className="hover:bg-app-hover flex w-full items-center gap-2 rounded-md px-2 py-1 text-left"
							>
								<span className="text-ink-dull min-w-0 flex-1 truncate text-sm">
									{child.name}
								</span>
								<span className="text-ink-faint shrink-0 text-xs tabular-nums">
									{formatBytes(child.size)}
								</span>
							</button>
						))}
						{root.other_size > 0 && (
							<div className="flex w-full items-center gap-2 px-2 py-1">
								<span className="text-ink-faint min-w-0 flex-1 truncate text-sm">
									everything else
								</span>
								<span className="text-ink-faint shrink-0 text-xs tabular-nums">
									{formatBytes(root.other_size)}
								</span>
							</div>
						)}
					</div>
				)}
			</div>
		</div>
	);
}
