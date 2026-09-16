import {useState} from 'react';
import type {ContentKind, SearchFacets, SearchFilters} from '@sd/ts-client';
import clsx from 'clsx';

/** A size preset the panel offers; null bounds are open-ended. */
const SIZE_PRESETS: Array<{
	label: string;
	min: number | null;
	max: number | null;
}> = [
	{label: 'Any size', min: null, max: null},
	{label: '< 1 MB', min: null, max: 1_000_000},
	{label: '1–100 MB', min: 1_000_000, max: 100_000_000},
	{label: '> 100 MB', min: 100_000_000, max: null}
];

const DATE_PRESETS: Array<{label: string; days: number | null}> = [
	{label: 'Any time', days: null},
	{label: 'Today', days: 1},
	{label: 'Last 7 days', days: 7},
	{label: 'Last 30 days', days: 30},
	{label: 'This year', days: 365}
];

/** How many kind options the panel offers, most common first. */
const KIND_OPTION_LIMIT = 8;

interface SearchFiltersPanelProps {
	filters: SearchFilters;
	facets?: SearchFacets;
	onChange: (filters: SearchFilters) => void;
}

/**
 * The refinement panel under the search toolbar. Every control maps onto a
 * wire filter the daemon advertises as answerable; nothing here can pass
 * silently. Kind options come from the facets of the current match set.
 */
export function SearchFiltersPanel({
	filters,
	facets,
	onChange
}: SearchFiltersPanelProps) {
	const [extensionDraft, setExtensionDraft] = useState(
		filters.file_types?.join(', ') ?? ''
	);

	const kinds: Array<[string, number]> = Object.entries(facets?.kinds ?? {})
		.sort((a, b) => b[1] - a[1])
		.slice(0, KIND_OPTION_LIMIT);
	const selectedKinds = filters.content_types ?? [];

	const toggleKind = (kind: string) => {
		const wire = kind as ContentKind;
		const next = selectedKinds.includes(wire)
			? selectedKinds.filter((k) => k !== wire)
			: [...selectedKinds, wire];
		onChange({...filters, content_types: next.length > 0 ? next : null});
	};

	const applyExtensions = (draft: string) => {
		const extensions = draft
			.split(',')
			.map((ext) => ext.trim().replace(/^\./, '').toLowerCase())
			.filter(Boolean);
		onChange({
			...filters,
			file_types: extensions.length > 0 ? extensions : null
		});
	};

	const activeSizePreset = SIZE_PRESETS.findIndex(
		(preset) =>
			(filters.size_range?.min ?? null) === preset.min &&
			(filters.size_range?.max ?? null) === preset.max
	);

	const applySizePreset = (index: number) => {
		const preset = SIZE_PRESETS[index];
		onChange({
			...filters,
			size_range:
				preset.min === null && preset.max === null
					? null
					: {min: preset.min, max: preset.max}
		});
	};

	const activeDateDays = (() => {
		if (!filters.date_range?.start) return null;
		const elapsed = Date.now() - new Date(filters.date_range.start).getTime();
		const days = Math.round(elapsed / 86_400_000);
		return DATE_PRESETS.find((p) => p.days === days)?.days ?? days;
	})();

	const applyDatePreset = (days: number | null) => {
		onChange({
			...filters,
			date_range:
				days === null
					? null
					: {
							field: 'ModifiedAt',
							start: new Date(
								Date.now() - days * 86_400_000
							).toISOString(),
							end: null
						}
		});
	};

	return (
		<div className="flex flex-wrap items-center gap-x-6 gap-y-2 px-4 py-2 border-b border-sidebar-line/30 bg-sidebar-box/5">
			{kinds.length > 0 && (
				<div className="flex items-center gap-1.5">
					<span className="text-xs text-sidebar-inkDull">Kind</span>
					{kinds.map(([kind, count]) => (
						<FilterChip
							key={kind}
							active={selectedKinds.includes(kind as ContentKind)}
							onClick={() => toggleKind(kind)}
						>
							{kind} · {count.toLocaleString()}
						</FilterChip>
					))}
				</div>
			)}

			<label className="flex items-center gap-1.5 text-xs text-sidebar-inkDull">
				Extension
				<input
					value={extensionDraft}
					onChange={(e) => setExtensionDraft(e.target.value)}
					onBlur={() => applyExtensions(extensionDraft)}
					onKeyDown={(e) => {
						if (e.key === 'Enter') applyExtensions(extensionDraft);
					}}
					placeholder="jpg, pdf"
					className="w-24 rounded-md bg-sidebar-box/40 px-2 py-0.5 text-xs text-sidebar-ink placeholder:text-sidebar-inkFaint focus:outline-none focus:ring-1 focus:ring-accent"
				/>
			</label>

			<div className="flex items-center gap-1.5">
				<span className="text-xs text-sidebar-inkDull">Size</span>
				{SIZE_PRESETS.map((preset, index) => (
					<FilterChip
						key={preset.label}
						active={
							preset.min === null && preset.max === null
								? !filters.size_range
								: activeSizePreset === index
						}
						onClick={() => applySizePreset(index)}
					>
						{preset.label}
					</FilterChip>
				))}
			</div>

			<div className="flex items-center gap-1.5">
				<span className="text-xs text-sidebar-inkDull">Modified</span>
				{DATE_PRESETS.map((preset) => (
					<FilterChip
						key={preset.label}
						active={
							preset.days === null
								? filters.date_range === null
								: activeDateDays === preset.days
						}
						onClick={() => applyDatePreset(preset.days)}
					>
						{preset.label}
					</FilterChip>
				))}
			</div>

			<label className="flex items-center gap-1.5 text-xs text-sidebar-inkDull cursor-pointer">
				<input
					type="checkbox"
					checked={filters.include_hidden === true}
					onChange={(e) =>
						onChange({
							...filters,
							include_hidden: e.target.checked ? true : null
						})
					}
					className="accent-accent"
				/>
				Hidden files
			</label>
		</div>
	);
}

function FilterChip({
	active,
	onClick,
	children
}: {
	active: boolean;
	onClick: () => void;
	children: React.ReactNode;
}) {
	return (
		<button
			onClick={onClick}
			className={clsx(
				'rounded-full px-2 py-0.5 text-xs font-medium transition-colors',
				active
					? 'bg-accent text-white'
					: 'bg-sidebar-box/40 text-sidebar-inkDull hover:text-sidebar-ink hover:bg-sidebar-selected/40'
			)}
		>
			{children}
		</button>
	);
}
