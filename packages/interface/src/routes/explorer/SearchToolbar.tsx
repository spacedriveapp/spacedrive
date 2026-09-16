import {X} from '@phosphor-icons/react';
import type {ContentKind, SearchFacets, SearchFilters} from '@sd/ts-client';
import {useLibraryQuery} from '@sd/ts-client';
import {
	Button,
	Input,
	OptionList,
	OptionListItem,
	Popover,
	SelectPill,
	Switch,
	ToggleGroup,
	usePopover
} from '@spacedrive/primitives';
import clsx from 'clsx';
import {useState} from 'react';
import {useExplorer} from './context';
import type {SearchScope} from './context';
import {useExplorerFiles} from './hooks/useExplorerFiles';

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

/** How many kind options the pill offers, most common first. */
const KIND_OPTION_LIMIT = 10;

/**
 * The search refinement bar: scope, filter pills, hidden toggle, and the
 * true result count, in one row of primitives. A pill carries its own state
 * in its label, so an active filter is legible without opening anything.
 */
export function SearchToolbar() {
	const explorer = useExplorer();
	const isSearching = explorer.mode.type === 'search';
	const {totalFound, isLoading, facets} = useExplorerFiles();

	// The containing source, resolved the same way the path bar resolves it.
	// A location is a pin and pins do not define search domains; a source
	// does, and paths.context reports it alias-normalized.
	const {data: pathContext} = useLibraryQuery(
		{
			type: 'paths.context',
			input: {path: explorer.currentPath!}
		},
		{enabled: isSearching && !!explorer.currentPath}
	);

	if (explorer.mode.type !== 'search') {
		return null;
	}

	const {scope} = explorer.mode;
	const source = pathContext?.source ?? null;
	const filters = explorer.searchFilters;

	const scopeOptions = [
		{value: 'folder', label: 'This Folder'},
		...(source ? [{value: 'source', label: source.name}] : []),
		{value: 'library', label: 'Library'}
	];

	const handleScopeChange = (value: string) => {
		if (explorer.mode.type === 'search') {
			explorer.enterSearchMode(explorer.mode.query, value as SearchScope);
		}
	};

	return (
		<div className="flex flex-wrap items-center gap-2 px-3 py-1.5 border-b border-app-line/50">
			<ToggleGroup
				size="sm"
				options={scopeOptions}
				value={scope === 'source' && !source ? 'folder' : scope}
				onChange={handleScopeChange}
			/>

			<div className="mx-1 h-4 w-px bg-app-line/60" />

			<KindPill
				filters={filters}
				facets={facets}
				onChange={explorer.setSearchFilters}
			/>
			<SizePill filters={filters} onChange={explorer.setSearchFilters} />
			<DatePill filters={filters} onChange={explorer.setSearchFilters} />
			<ExtensionPill
				filters={filters}
				onChange={explorer.setSearchFilters}
			/>

			<label className="ml-1 flex items-center gap-1.5 text-xs text-ink-dull cursor-pointer">
				<Switch
					size="sm"
					checked={filters.include_hidden === true}
					onCheckedChange={(checked) =>
						explorer.setSearchFilters({
							...filters,
							include_hidden: checked ? true : null
						})
					}
				/>
				Hidden
			</label>

			<div className="flex-1" />

			{totalFound != null && !isLoading && (
				<span className="text-xs text-ink-faint tabular-nums">
					{totalFound.toLocaleString()} result
					{totalFound === 1 ? '' : 's'}
				</span>
			)}

			<Button
				variant="subtle"
				size="xs"
				onClick={explorer.exitSearchMode}
			>
				<X className="size-3" weight="bold" />
				Clear
			</Button>
		</div>
	);
}

interface PillProps {
	filters: SearchFilters;
	onChange: (filters: SearchFilters) => void;
}

/** Shared pill styling: quiet at rest, accent-tinted while filtering. */
function pillClass(active: boolean): string {
	return clsx(active && 'text-accent [&_svg]:text-accent');
}

function KindPill({
	filters,
	facets,
	onChange
}: PillProps & {facets?: SearchFacets}) {
	const popover = usePopover();
	const selected = filters.content_types ?? [];

	const options: Array<[string, number]> = Object.entries(
		facets?.kinds ?? {}
	)
		.sort((a, b) => b[1] - a[1])
		.slice(0, KIND_OPTION_LIMIT);

	const label =
		selected.length === 0
			? 'Kind'
			: selected.length === 1
				? selected[0]
				: `${selected[0]} +${selected.length - 1}`;

	const toggle = (kind: string) => {
		const wire = kind as ContentKind;
		const next = selected.includes(wire)
			? selected.filter((k) => k !== wire)
			: [...selected, wire];
		onChange({...filters, content_types: next.length > 0 ? next : null});
	};

	return (
		<Popover.Root open={popover.open} onOpenChange={popover.setOpen}>
			<Popover.Trigger asChild>
				<SelectPill size="sm" className={pillClass(selected.length > 0)}>
					{label}
				</SelectPill>
			</Popover.Trigger>
			<Popover.Content align="start" sideOffset={8}>
				<OptionList>
					<OptionListItem
						selected={selected.length === 0}
						onClick={() => {
							onChange({...filters, content_types: null});
							popover.setOpen(false);
						}}
					>
						Any kind
					</OptionListItem>
					{options.map(([kind, count]) => (
						<OptionListItem
							key={kind}
							selected={selected.includes(kind as ContentKind)}
							onClick={() => toggle(kind)}
						>
							<span className="flex w-full items-center justify-between gap-4">
								{kind}
								<span className="text-ink-faint tabular-nums">
									{count.toLocaleString()}
								</span>
							</span>
						</OptionListItem>
					))}
				</OptionList>
			</Popover.Content>
		</Popover.Root>
	);
}

function SizePill({filters, onChange}: PillProps) {
	const popover = usePopover();
	const activeIndex = filters.size_range
		? SIZE_PRESETS.findIndex(
				(preset) =>
					(filters.size_range?.min ?? null) === preset.min &&
					(filters.size_range?.max ?? null) === preset.max
			)
		: 0;

	return (
		<Popover.Root open={popover.open} onOpenChange={popover.setOpen}>
			<Popover.Trigger asChild>
				<SelectPill size="sm" className={pillClass(activeIndex > 0)}>
					{activeIndex > 0 ? SIZE_PRESETS[activeIndex].label : 'Size'}
				</SelectPill>
			</Popover.Trigger>
			<Popover.Content align="start" sideOffset={8}>
				<OptionList>
					{SIZE_PRESETS.map((preset, index) => (
						<OptionListItem
							key={preset.label}
							selected={index === activeIndex}
							onClick={() => {
								onChange({
									...filters,
									size_range:
										index === 0
											? null
											: {min: preset.min, max: preset.max}
								});
								popover.setOpen(false);
							}}
						>
							{preset.label}
						</OptionListItem>
					))}
				</OptionList>
			</Popover.Content>
		</Popover.Root>
	);
}

function DatePill({filters, onChange}: PillProps) {
	const popover = usePopover();

	const activeDays = (() => {
		if (!filters.date_range?.start) return null;
		const elapsed =
			Date.now() - new Date(filters.date_range.start).getTime();
		return Math.round(elapsed / 86_400_000);
	})();
	const activePreset = DATE_PRESETS.find((p) => p.days === activeDays);

	return (
		<Popover.Root open={popover.open} onOpenChange={popover.setOpen}>
			<Popover.Trigger asChild>
				<SelectPill
					size="sm"
					className={pillClass(filters.date_range !== null)}
				>
					{filters.date_range
						? (activePreset?.label ?? 'Custom')
						: 'Modified'}
				</SelectPill>
			</Popover.Trigger>
			<Popover.Content align="start" sideOffset={8}>
				<OptionList>
					{DATE_PRESETS.map((preset) => (
						<OptionListItem
							key={preset.label}
							selected={
								preset.days === null
									? filters.date_range === null
									: activeDays === preset.days
							}
							onClick={() => {
								onChange({
									...filters,
									date_range:
										preset.days === null
											? null
											: {
													field: 'ModifiedAt',
													start: new Date(
														Date.now() -
															preset.days *
																86_400_000
													).toISOString(),
													end: null
												}
								});
								popover.setOpen(false);
							}}
						>
							{preset.label}
						</OptionListItem>
					))}
				</OptionList>
			</Popover.Content>
		</Popover.Root>
	);
}

function ExtensionPill({filters, onChange}: PillProps) {
	const popover = usePopover();
	const [draft, setDraft] = useState(filters.file_types?.join(', ') ?? '');
	const active = (filters.file_types?.length ?? 0) > 0;

	const apply = () => {
		const extensions = draft
			.split(',')
			.map((ext) => ext.trim().replace(/^\./, '').toLowerCase())
			.filter(Boolean);
		onChange({
			...filters,
			file_types: extensions.length > 0 ? extensions : null
		});
		popover.setOpen(false);
	};

	return (
		<Popover.Root open={popover.open} onOpenChange={popover.setOpen}>
			<Popover.Trigger asChild>
				<SelectPill size="sm" className={pillClass(active)}>
					{active
						? filters.file_types!.map((ext) => `.${ext}`).join(' ')
						: 'Extension'}
				</SelectPill>
			</Popover.Trigger>
			<Popover.Content align="start" sideOffset={8} className="p-2">
				<Input
					autoFocus
					value={draft}
					onChange={(e) => setDraft(e.target.value)}
					onKeyDown={(e) => {
						if (e.key === 'Enter') apply();
					}}
					onBlur={apply}
					placeholder="jpg, pdf, mp4"
					className="w-40"
				/>
			</Popover.Content>
		</Popover.Root>
	);
}
