/**
 * At-Risk / Redundant Files View
 *
 * Enters the Explorer into "filtered" mode with a redundancy SearchFilters
 * payload, then delegates rendering to the real ExplorerView so users get
 * the full file browser experience (view modes, selection, QuickPreview,
 * context menus, drag-drop, sort, etc.).
 */

import {ArrowLeft, ShieldWarning} from '@phosphor-icons/react';
import type {SearchFilters} from '@sd/ts-client';
import {CircleButton} from '@spacedrive/primitives';
import {useEffect, useMemo, useRef} from 'react';
import {useNavigate, useSearchParams} from 'react-router-dom';
import {TopBarItem, TopBarPortal} from '../../TopBar';
import {ExplorerView, useExplorer} from '../explorer';

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

export function AtRiskFiles() {
	const navigate = useNavigate();
	const [searchParams] = useSearchParams();
	const volumeFilter = searchParams.get('volume');
	const atRiskParam = searchParams.get('at_risk');
	const isAtRisk = atRiskParam !== 'false'; // default true

	const {enterFilteredMode, exitFilteredMode, setSortBy} = useExplorer();

	const label = isAtRisk ? 'At-Risk Files' : 'Redundant Files';

	const filters = useMemo<SearchFilters>(
		() => ({
			...EMPTY_FILTERS,
			at_risk: isAtRisk,
			on_volumes: volumeFilter ? [volumeFilter] : null
		}),
		[isAtRisk, volumeFilter]
	);

	// Keep filtered mode in sync with current params
	useEffect(() => {
		enterFilteredMode(filters, label);
	}, [enterFilteredMode, filters, label]);

	// Default sort to size (largest first) on mount — biggest risk/dupe first.
	// Guarded with a ref because setSortBy's identity churns whenever the
	// explorer context's derived deps update, which would otherwise loop.
	const didSetSort = useRef(false);
	useEffect(() => {
		if (didSetSort.current) return;
		didSetSort.current = true;
		setSortBy('size');
	}, [setSortBy]);

	// Exit filtered mode only when the route unmounts
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
				<ShieldWarning
					size={18}
					weight="bold"
					className="text-status-warning"
				/>
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
						id="redundancy-at-risk-title"
						label={label}
						priority="high"
					>
						{titleItem}
					</TopBarItem>
				}
			/>
			<ExplorerView />
		</>
	);
}
