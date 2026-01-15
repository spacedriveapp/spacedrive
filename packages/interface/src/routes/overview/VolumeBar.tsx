import {Database, Plus} from '@phosphor-icons/react';
import DatabaseIcon from '@sd/assets/icons/Database.png';
import DriveAmazonS3Icon from '@sd/assets/icons/Drive-AmazonS3.png';
import DriveDropboxIcon from '@sd/assets/icons/Drive-Dropbox.png';
import DriveGoogleDriveIcon from '@sd/assets/icons/Drive-GoogleDrive.png';
import DriveIcon from '@sd/assets/icons/Drive.png';
import HDDIcon from '@sd/assets/icons/HDD.png';
import ServerIcon from '@sd/assets/icons/Server.png';
import type {Device, VolumeItem} from '@sd/ts-client';
import {motion} from 'framer-motion';
import {useEffect, useState} from 'react';
import {
	useLibraryMutation,
	useNormalizedQuery,
	useSpacedriveClient
} from '../../contexts/SpacedriveContext';
import {useVolumeIndexingStore} from '../../stores/volumeIndexingStore';
import {formatBytes, getVolumeIcon} from './DevicePanel';

function getDiskTypeLabel(diskType: string): string {
	return diskType === 'SSD' ? 'SSD' : diskType === 'HDD' ? 'HDD' : diskType;
}

interface VolumeBarProps {
	volume: VolumeItem;
	index: number;
}

interface IndexingProgress {
	filesIndexed: number;
	bytesIndexed: number;
	percentage: number;
	rate: number;
}

export function VolumeBar({volume, index}: VolumeBarProps) {
	const trackVolume = useLibraryMutation('volumes.track');
	const indexVolume = useLibraryMutation('volumes.index');
	const [indexingProgress, setIndexingProgress] =
		useState<IndexingProgress | null>(null);
	const client = useSpacedriveClient();

	// Get the job ID for this volume from the store
	const jobId = useVolumeIndexingStore((state) =>
		state.getJobId(volume.fingerprint)
	);

	// Subscribe to job events for this volume
	useEffect(() => {
		if (!client) return;

		let unsubscribe: (() => void) | undefined;
		let isCancelled = false;

		const handleEvent = (event: any) => {
			const eventType = Object.keys(event)[0];

			// Client-side filter: only handle job events
			if (
				![
					'JobProgress',
					'JobCompleted',
					'JobFailed',
					'JobCancelled'
				].includes(eventType)
			) {
				return; // Skip unwanted events
			}

			if ('JobProgress' in event) {
				const progressData = event.JobProgress;
				if (!progressData) return;

				// Read the current job ID from store (avoids stale closure)
				const currentJobId = useVolumeIndexingStore
					.getState()
					.getJobId(volume.fingerprint);

				// Only handle progress for this volume's job
				if (progressData.job_id !== currentJobId) return;

				const generic = progressData.generic_progress;
				if (!generic) return;

				setIndexingProgress({
					filesIndexed: generic.completion?.completed || 0,
					bytesIndexed: generic.completion?.bytes_completed || 0,
					percentage: generic.percentage || 0,
					rate: generic.performance?.rate || 0
				});
			} else if (
				'JobCompleted' in event ||
				'JobFailed' in event ||
				'JobCancelled' in event
			) {
				const eventJobId =
					event.JobCompleted?.job_id ||
					event.JobFailed?.job_id ||
					event.JobCancelled?.job_id;

				const currentJobId = useVolumeIndexingStore
					.getState()
					.getJobId(volume.fingerprint);

				if (eventJobId === currentJobId) {
					setIndexingProgress(null);
				}
			}
		};

		const filter = {
			event_types: [
				'JobProgress',
				'JobCompleted',
				'JobFailed',
				'JobCancelled'
			],
			// Ensure unique subscription key to prevent multiplexing with resource subscriptions
			resource_type: undefined,
			path_scope: undefined,
			library_id: undefined
		};

		client.subscribeFiltered(filter, handleEvent).then((unsub) => {
			if (isCancelled) {
				unsub();
			} else {
				unsubscribe = unsub;
			}
		});

		return () => {
			isCancelled = true;
			unsubscribe?.();
		};
	}, [client, volume.fingerprint]);

	// Get current device to check if this volume is local
	const devicesQuery = useNormalizedQuery<any, Device[]>({
		wireMethod: 'query:devices.list',
		input: {include_offline: true, include_details: false},
		resourceType: 'device'
	});

	const currentDevice = devicesQuery.data?.find((d) => d.is_current);

	const handleTrack = async () => {
		try {
			await trackVolume.mutateAsync({
				fingerprint: volume.fingerprint
			});
		} catch (error) {
			console.error('Failed to track volume:', error);
		}
	};

	const handleIndex = async () => {
		try {
			const result = await indexVolume.mutateAsync({
				fingerprint: volume.fingerprint,
				scope: 'Recursive'
			});
			console.log('Volume indexed:', result.message);
		} catch (error) {
			console.error('Failed to index volume:', error);
		}
	};

	if (!volume.total_capacity) {
		return null;
	}

	const totalCapacity = volume.total_capacity;
	const availableBytes = volume.available_capacity || 0;
	const usedBytes = totalCapacity - availableBytes;

	const uniqueBytes = volume.unique_bytes ?? Math.floor(usedBytes * 0.7);
	const duplicateBytes = usedBytes - uniqueBytes;

	const uniquePercent = (uniqueBytes / totalCapacity) * 100;
	const duplicatePercent = (duplicateBytes / totalCapacity) * 100;

	// Convert enum values to strings for safe rendering
	const fileSystem = volume.file_system
		? typeof volume.file_system === 'string'
			? volume.file_system
			: (volume.file_system as any)?.Other ||
				JSON.stringify(volume.file_system)
		: 'Unknown';
	const diskType = volume.disk_type
		? typeof volume.disk_type === 'string'
			? volume.disk_type
			: (volume.disk_type as any)?.Other ||
				JSON.stringify(volume.disk_type)
		: 'Unknown';

	const iconSrc = getVolumeIcon(volume.volume_type, volume.name);
	const volumeTypeStr =
		typeof volume.volume_type === 'string'
			? volume.volume_type
			: (volume.volume_type as any)?.Other ||
				JSON.stringify(volume.volume_type);

	return (
		<motion.div
			initial={{opacity: 0, y: 10}}
			animate={{opacity: 1, y: 0}}
			transition={{delay: index * 0.05}}
			className="bg-app-box border-app-line/50 overflow-hidden rounded-lg border"
		>
			{/* Top row: Info */}
			<div className="flex items-center gap-3 px-3 py-2">
				{/* Icon */}
				<img
					src={iconSrc}
					alt={volumeTypeStr}
					className="size-6 flex-shrink-0 opacity-80"
				/>

				{/* Name, actions, and badges */}
				<div className="min-w-0 flex-1">
					<div className="mb-1 flex items-center gap-2">
						<span className="text-ink truncate text-sm font-semibold">
							{volume.display_name || volume.name}
						</span>
						{!volume.is_online && (
							<span className="bg-app-box text-ink-faint border-app-line rounded border px-1.5 py-0.5 text-[10px]">
								Offline
							</span>
						)}
						{!volume.is_tracked && (
							<button
								onClick={handleTrack}
								disabled={trackVolume.isPending}
								className="bg-accent/10 hover:bg-accent/20 text-accent border-accent/20 hover:border-accent/30 flex items-center gap-1 rounded border px-1.5 py-0.5 text-[10px] transition-colors disabled:opacity-50"
								title="Track this volume"
							>
								<Plus className="size-2.5" weight="bold" />
								{trackVolume.isPending
									? 'Tracking...'
									: 'Track'}
							</button>
						)}
						{currentDevice &&
							volume.device_id === currentDevice.id && (
								<button
									onClick={handleIndex}
									disabled={indexVolume.isPending}
									className="bg-sidebar-box hover:bg-sidebar-selected text-sidebar-ink border-sidebar-line flex items-center gap-1 rounded border px-1.5 py-0.5 text-[10px] transition-colors disabled:opacity-50"
									title="Index this volume"
								>
									<Database
										className="size-2.5"
										weight="bold"
									/>
									{indexVolume.isPending
										? 'Indexing...'
										: 'Index'}
								</button>
							)}
					</div>

					{/* Badges under name */}
					<div className="text-ink-dull flex flex-wrap items-center gap-1.5 text-[10px]">
						<span className="bg-app-box border-app-line rounded border px-1.5 py-0.5">
							{fileSystem}
						</span>
						<span className="bg-app-box border-app-line rounded border px-1.5 py-0.5">
							{getDiskTypeLabel(diskType)}
						</span>
						<span className="bg-app-box border-app-line rounded border px-1.5 py-0.5">
							{volumeTypeStr}
						</span>
						{indexingProgress ? (
							<span className="bg-accent/20 border-accent/30 text-accent rounded border px-1.5 py-0.5 font-medium">
								{indexingProgress.filesIndexed.toLocaleString()}{' '}
								files
								{indexingProgress.rate > 0 && (
									<span className="text-accent/70 ml-1">
										({Math.round(indexingProgress.rate)}/s)
									</span>
								)}
							</span>
						) : (
							volume.total_file_count != null && (
								<span className="bg-accent/10 border-accent/20 text-accent rounded border px-1.5 py-0.5">
									{volume.total_file_count.toLocaleString()}{' '}
									files
								</span>
							)
						)}
					</div>
				</div>

				{/* Capacity info */}
				<div className="flex-shrink-0 text-right">
					<div className="text-ink text-sm font-medium">
						{formatBytes(totalCapacity)}
					</div>
					<div className="text-ink-dull text-[10px]">
						{formatBytes(availableBytes)} free
					</div>
				</div>
			</div>

			{/* Bottom: Full-width capacity bar with padding */}
			<div className="px-3 pb-3 pt-2">
				<div className="bg-app border-app-line relative h-8 overflow-hidden rounded-md border">
					{/* Base capacity visualization */}
					<div className="flex h-full">
						<motion.div
							initial={{width: 0}}
							animate={{width: `${uniquePercent}%`}}
							transition={{
								duration: 1,
								ease: 'easeOut',
								delay: index * 0.05
							}}
							className="bg-accent border-accent-deep border-r"
							title={`Unique: ${formatBytes(uniqueBytes)} (${uniquePercent.toFixed(1)}%)`}
						/>
						<motion.div
							initial={{width: 0}}
							animate={{width: `${duplicatePercent}%`}}
							transition={{
								duration: 1,
								ease: 'easeOut',
								delay: index * 0.05 + 0.2
							}}
							className="bg-accent/60"
							style={{
								backgroundImage:
									'repeating-linear-gradient(45deg, transparent, transparent 4px, rgba(255,255,255,0.1) 4px, rgba(255,255,255,0.1) 8px)'
							}}
							title={`Duplicate: ${formatBytes(duplicateBytes)} (${duplicatePercent.toFixed(1)}%)`}
						/>
					</div>

					{/* Indexing progress overlay */}
					{indexingProgress && (
						<motion.div
							initial={{width: 0}}
							animate={{
								width: `${(indexingProgress.bytesIndexed / totalCapacity) * 100}%`
							}}
							transition={{duration: 0.3, ease: 'easeOut'}}
							className="bg-accent-deep border-accent-deep absolute inset-y-0 left-0 border-r-2"
							title={`Indexing: ${formatBytes(indexingProgress.bytesIndexed)} / ${formatBytes(totalCapacity)} (${(indexingProgress.percentage * 100).toFixed(1)}%)`}
						>
							{/* Animated shimmer effect */}
							<div
								className="absolute inset-0 opacity-30"
								style={{
									backgroundImage:
										'linear-gradient(90deg, transparent 0%, rgba(255,255,255,0.4) 50%, transparent 100%)',
									backgroundSize: '200% 100%',
									animation: 'shimmer 2s infinite'
								}}
							/>
						</motion.div>
					)}

					{/* Center label showing indexing status */}
					{indexingProgress && (
						<div className="absolute inset-0 flex items-center justify-center">
							<span className="text-ink text-xs font-medium drop-shadow-lg">
								Indexing:{' '}
								{(indexingProgress.percentage * 100).toFixed(1)}
								%
								<span className="text-ink-dull ml-2">
									{formatBytes(indexingProgress.bytesIndexed)}{' '}
									/ {formatBytes(totalCapacity)}
								</span>
							</span>
						</div>
					)}
				</div>
			</div>
		</motion.div>
	);
}
