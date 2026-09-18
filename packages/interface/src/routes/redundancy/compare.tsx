/**
 * Volume Comparison View
 *
 * Pick two volumes and see which files are unique to each or shared.
 * Once both volumes are selected, switches the Explorer into filtered mode
 * with the appropriate SearchFilters — the real ExplorerView renders the
 * results so users keep selection, QuickPreview, context menus, drag-drop, etc.
 */

import {ArrowLeft, ArrowsLeftRight, ShieldCheck} from '@phosphor-icons/react';
import type {SearchFilters} from '@sd/ts-client';
import {CircleButton} from '@spacedrive/primitives';
import {useEffect, useMemo, useRef, useState} from 'react';
import {useNavigate} from 'react-router-dom';
import {useLibraryQuery} from '../../contexts/SpacedriveContext';
import {TopBarItem, TopBarPortal} from '../../TopBar';
import {ExplorerView, useExplorer} from '../explorer';

type CompareMode = 'unique_a' | 'shared' | 'unique_b';

const EMPTY_FILTERS: SearchFilters = {
	file_types: null,
	tags: null,
	date_range: null,
	size_range: null,
	content_types: null,
	include_hidden: null,
	include_archived: null,
	at_risk: null,
	on_volumes: null,
	not_on_volumes: null,
	min_volume_count: null,
	max_volume_count: null
};

function formatBytes(bytes: number): string {
	if (bytes === 0) return '0 B';
	const k = 1024;
	const sizes = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}

export function CompareVolumes() {
	const navigate = useNavigate();
	const [volumeA, setVolumeA] = useState<string | null>(null);
	const [volumeB, setVolumeB] = useState<string | null>(null);
	const [mode, setMode] = useState<CompareMode>('unique_a');

	const {enterFilteredMode, exitFilteredMode, setSortBy} = useExplorer();

	const {data: summaryData} = useLibraryQuery({
		type: 'redundancy.summary',
		input: {}
	});

	const volumes = summaryData?.volumes ?? [];

	const volumeAName =
		volumes.find((v) => v.volume_uuid === volumeA)?.display_name ??
		'Volume A';
	const volumeBName =
		volumes.find((v) => v.volume_uuid === volumeB)?.display_name ??
		'Volume B';

	const hasBoth = !!volumeA && !!volumeB;

	const filters = useMemo<SearchFilters | null>(() => {
		if (!hasBoth) return null;
		switch (mode) {
			case 'unique_a':
				return {
					...EMPTY_FILTERS,
					on_volumes: [volumeA!],
					not_on_volumes: [volumeB!]
				};
			case 'unique_b':
				return {
					...EMPTY_FILTERS,
					on_volumes: [volumeB!],
					not_on_volumes: [volumeA!]
				};
			case 'shared':
				return {
					...EMPTY_FILTERS,
					on_volumes: [volumeA!, volumeB!],
					min_volume_count: 2
				};
		}
	}, [hasBoth, mode, volumeA, volumeB]);

	const label = useMemo(() => {
		if (!hasBoth) return 'Compare Volumes';
		switch (mode) {
			case 'unique_a':
				return `Unique to ${volumeAName}`;
			case 'unique_b':
				return `Unique to ${volumeBName}`;
			case 'shared':
				return `Shared between ${volumeAName} & ${volumeBName}`;
		}
	}, [hasBoth, mode, volumeAName, volumeBName]);

	// Sync filtered mode whenever selection changes (only when both selected)
	useEffect(() => {
		if (filters) {
			enterFilteredMode(filters, label);
		} else {
			// Ensure we're not stuck in an old filtered state if user clears a picker
			exitFilteredMode();
		}
	}, [enterFilteredMode, exitFilteredMode, filters, label]);

	// Default sort to size (largest first) on mount. Guarded with a ref
	// because setSortBy's identity churns when context deps update.
	const didSetSort = useRef(false);
	useEffect(() => {
		if (didSetSort.current) return;
		didSetSort.current = true;
		setSortBy('size');
	}, [setSortBy]);

	// Exit filtered mode when leaving the route
	useEffect(() => {
		return () => exitFilteredMode();
	}, [exitFilteredMode]);

	const titleItem = useMemo(
		() => (
			<div className="flex items-center gap-2">
				<CircleButton
					icon={ArrowLeft}
					title="Back to Protection"
					onClick={() => navigate('/redundancy')}
				/>
				<ArrowsLeftRight size={18} weight="bold" className="text-ink" />
				<span className="text-ink text-sm font-semibold">{label}</span>
			</div>
		),
		[navigate, label]
	);

	return (
		<>
			<TopBarPortal
				center={
					<TopBarItem
						id="redundancy-compare-title"
						label="Compare Volumes"
						priority="high"
					>
						{titleItem}
					</TopBarItem>
				}
			/>

			<div className="flex h-full flex-col overflow-hidden">
				{/* Picker + mode toggle */}
				<div className="border-app-line bg-app-box/30 space-y-2 border-b p-3">
					<div className="flex items-center gap-2">
						<select
							value={volumeA ?? ''}
							onChange={(e) => setVolumeA(e.target.value || null)}
							className="border-app-line bg-app-box/50 text-ink flex-1 rounded-lg border px-3 py-1.5 text-sm"
						>
							<option value="">Select Volume A</option>
							{volumes.map((v) => (
								<option
									key={v.volume_uuid}
									value={v.volume_uuid}
									disabled={v.volume_uuid === volumeB}
								>
									{v.display_name ?? v.volume_uuid} (
									{formatBytes(v.total_bytes)})
								</option>
							))}
						</select>
						<ArrowsLeftRight
							size={16}
							className="text-ink-dull flex-shrink-0"
						/>
						<select
							value={volumeB ?? ''}
							onChange={(e) => setVolumeB(e.target.value || null)}
							className="border-app-line bg-app-box/50 text-ink flex-1 rounded-lg border px-3 py-1.5 text-sm"
						>
							<option value="">Select Volume B</option>
							{volumes.map((v) => (
								<option
									key={v.volume_uuid}
									value={v.volume_uuid}
									disabled={v.volume_uuid === volumeA}
								>
									{v.display_name ?? v.volume_uuid} (
									{formatBytes(v.total_bytes)})
								</option>
							))}
						</select>
					</div>

					{hasBoth && (
						<div className="border-app-line bg-app-box/50 flex gap-1 rounded-lg border p-1">
							<ModeButton
								active={mode === 'unique_a'}
								onClick={() => setMode('unique_a')}
								label={`Unique to ${volumeAName}`}
							/>
							<ModeButton
								active={mode === 'shared'}
								onClick={() => setMode('shared')}
								label="Shared"
							/>
							<ModeButton
								active={mode === 'unique_b'}
								onClick={() => setMode('unique_b')}
								label={`Unique to ${volumeBName}`}
							/>
						</div>
					)}
				</div>

				{/* Results */}
				<div className="flex-1 overflow-hidden">
					{!hasBoth ? (
						<div className="text-ink-dull flex flex-col items-center justify-center gap-2 py-16">
							<ShieldCheck size={48} weight="thin" />
							<span className="text-sm">
								Select two volumes to compare their contents
							</span>
						</div>
					) : (
						<ExplorerView />
					)}
				</div>
			</div>
		</>
	);
}

function ModeButton({
	active,
	onClick,
	label
}: {
	active: boolean;
	onClick: () => void;
	label: string;
}) {
	return (
		<button
			onClick={onClick}
			className={`flex-1 rounded-md px-3 py-1.5 text-xs font-medium transition-colors ${
				active ? 'bg-accent text-white' : 'text-ink-dull hover:text-ink'
			}`}
		>
			{label}
		</button>
	);
}
