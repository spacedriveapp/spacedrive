import {ArrowLeft, ChartPieSlice, Plus} from '@phosphor-icons/react';
import type {Device} from '@sd/ts-client';
import {CircleButton, SearchBar} from '@spacedrive/primitives';
import {useNavigate} from 'react-router-dom';
import {DriveSourceCard} from '../../components/Sources/DriveSourceCard';
import {SourceCard} from '../../components/Sources/SourceCard';
import {useTabManager} from '../../components/TabManager/useTabManager';
import {
	useCoreQuery,
	useLibraryQuery,
	useNormalizedQuery
} from '../../contexts/SpacedriveContext';
import {TopBarItem, TopBarPortal} from '../../TopBar';

export function SourcesHome() {
	const navigate = useNavigate();
	const {createTab} = useTabManager();
	const {
		data: sourcesRaw,
		isLoading,
		error
	} = useLibraryQuery({
		type: 'sources.list',
		input: {data_type: null}
	});
	const sources = sourcesRaw?.filter(
		(source) => source.data_type !== 'filesystem'
	);

	// Filesystem sources — drives are sources too; the registries converge
	// storage-side, the surface converges here.
	const {data: ephemeralStatus} = useCoreQuery({
		type: 'core.ephemeral_status',
		input: {}
	});
	const driveSources = ephemeralStatus?.sources ?? [];
	const {data: devicesData} = useNormalizedQuery({
		query: 'devices.list',
		input: {include_offline: true, include_details: false},
		resourceType: 'device'
	});
	const devices: Device[] = devicesData ?? [];
	const deviceSlug = devices.find((device) => device.is_current)?.slug;
	const totalCount = (sources?.length ?? 0) + driveSources.length;

	return (
		<>
			<TopBarPortal
				left={
					<>
						<TopBarItem id="back" label="Back" priority="high">
							<CircleButton
								icon={ArrowLeft}
								onClick={() => navigate(-1)}
							/>
						</TopBarItem>
						<TopBarItem id="title" label="Title" priority="high">
							<h1 className="text-ink text-xl font-bold">
								Storage
							</h1>
						</TopBarItem>
					</>
				}
				right={
					<>
						<TopBarItem id="search" label="Search" priority="high">
							<SearchBar
								placeholder="Search sources..."
								value=""
								onChange={() => {}}
								onClear={() => {}}
								className="w-64"
							/>
						</TopBarItem>
						<TopBarItem
							id="storage-usage"
							label="Usage"
							priority="normal"
						>
							<CircleButton
								icon={ChartPieSlice}
								onClick={() => navigate('/analyzer')}
								title="Storage usage"
							/>
						</TopBarItem>
						<TopBarItem
							id="add-source"
							label="Add Source"
							priority="high"
						>
							<CircleButton
								icon={Plus}
								onClick={() =>
									createTab('Adapters', '/sources/adapters')
								}
								title="Add Source"
							/>
						</TopBarItem>
					</>
				}
			/>
			<div className="p-6">
				{isLoading && (
					<div className="flex items-center justify-center py-20">
						<div className="text-ink-faint text-sm">Loading...</div>
					</div>
				)}

				{error && (
					<div className="rounded-lg border border-red-400/20 p-4">
						<p className="text-sm text-red-400">
							Failed to load sources: {String(error)}
						</p>
					</div>
				)}

				{sources && totalCount === 0 && (
					<div className="flex flex-col items-center justify-center py-20">
						<p className="text-ink-dull text-sm">No storage yet</p>
						<p className="text-ink-faint mt-1 text-xs">
							Track a folder, drive, server, or provider to get
							started
						</p>
						<button
							onClick={() =>
								createTab('Adapters', '/sources/adapters')
							}
							className="bg-accent hover:bg-accent-deep mt-4 rounded-lg px-3.5 py-1.5 text-sm font-medium text-white transition-colors"
						>
							Add Source
						</button>
					</div>
				)}

				{totalCount > 0 && (
					<div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
						{driveSources.map((source) => (
							<DriveSourceCard
								key={source.id}
								source={source}
								deviceSlug={deviceSlug}
							/>
						))}
						{(sources ?? []).map((source) => (
							<SourceCard key={source.id} source={source} />
						))}
					</div>
				)}
			</div>
		</>
	);
}
