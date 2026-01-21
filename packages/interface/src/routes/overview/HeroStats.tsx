import {Lightning} from '@phosphor-icons/react';
import ComputeIcon from '@sd/assets/icons/Compute.png';
import DevicesIcon from '@sd/assets/icons/Devices.png';
import IndexedIcon from '@sd/assets/icons/Indexed.png';
import MobileIcon from '@sd/assets/icons/Mobile.png';
import {motion} from 'framer-motion';

interface HeroStatsProps {
	totalStorage: number; // bytes
	usedStorage: number; // bytes
	totalFiles: number;
	locationCount: number;
	tagCount: number;
	deviceCount: number;
	uniqueContentCount: number;
}

function formatBytes(bytes: number): {value: string; unit: string} {
	if (bytes === 0) return {value: '0', unit: 'B'};
	const k = 1024;
	const sizes = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return {
		value: (bytes / Math.pow(k, i)).toFixed(1),
		unit: sizes[i]
	};
}

function getTOPSRank(tops: number): {label: string} {
	if (tops >= 100) return {label: 'Extreme'};
	if (tops >= 70) return {label: 'Very High'};
	if (tops >= 40) return {label: 'High'};
	if (tops >= 20) return {label: 'Moderate'};
	return {label: 'Low'};
}

export function HeroStats({
	totalStorage,
	usedStorage,
	totalFiles,
	locationCount,
	deviceCount,
	uniqueContentCount
}: HeroStatsProps) {
	const usagePercent =
		totalStorage > 0 ? (usedStorage / totalStorage) * 100 : 0;

	const storageFormatted = formatBytes(totalStorage);
	const usedFormatted = formatBytes(usedStorage);
	const topsValue = 70;
	const topsRank = getTOPSRank(topsValue);

	return (
		<div className="px-8 py-8">
			<div className="grid grid-cols-2 gap-8 lg:grid-cols-4">
				{/* Total Storage */}
				<StatCard
					icon={
						<img
							src={DevicesIcon}
							alt="Storage"
							className="size-10 opacity-80"
							// style={{ filter: 'drop-shadow(0 0 4px rgba(217, 70, 239, 0.4))' }}
						/>
					}
					label="Total Storage"
					value={
						<>
							{storageFormatted.value}{' '}
							<span className="text-ink-faint text-xl">
								{storageFormatted.unit}
							</span>
						</>
					}
					subtitle={
						<>
							<span className="text-accent">
								{usedFormatted.value}{' '}
								<span className="text-accent/70 text-[10px]">
									{usedFormatted.unit}
								</span>
							</span>{' '}
							used
						</>
					}
					progress={usagePercent}
					color="from-accent to-cyan-500"
				/>

				{/* Files */}
				<StatCard
					icon={
						<img
							src={IndexedIcon}
							alt="Files"
							className="size-10 opacity-80"
						/>
					}
					label="Files Indexed"
					value={totalFiles.toLocaleString()}
					subtitle={`${uniqueContentCount.toLocaleString()} unique files`}
					color="from-purple-500 to-pink-500"
				/>

				{/* Devices */}
				<StatCard
					icon={
						<img
							src={MobileIcon}
							alt="Devices"
							className="size-10 opacity-80"
						/>
					}
					label="Connected Devices"
					value={deviceCount}
					subtitle={`registered in library`}
					color="from-green-500 to-emerald-500"
				/>

				{/* Storage Health - Future feature */}
				<StatCard
					icon={
						<img
							src={ComputeIcon}
							alt="Compute"
							className="size-10 opacity-80"
						/>
					}
					label="AI Compute Power"
					value={
						<>
							{topsValue}{' '}
							<span className="text-ink-faint text-xl">TOPS</span>
						</>
					}
					subtitle={
						<span className="flex items-center gap-1">
							<Lightning
								size={12}
								weight="bold"
								className="text-ink-faint"
							/>
							{topsRank.label}
						</span>
					}
					color="from-purple-500 to-pink-500"
				/>
			</div>
		</div>
	);
}

interface StatCardProps {
	icon: React.ReactNode;
	label: string;
	value: string | number | React.ReactNode;
	subtitle: React.ReactNode;
	progress?: number;
	pulse?: boolean;
	color: string;
	badge?: string;
}

function StatCard({
	icon,
	label,
	value,
	subtitle,
	progress,
	pulse,
	color,
	badge
}: StatCardProps) {
	return (
		<div className="relative">
			{badge && (
				<div className="bg-sidebar-box text-sidebar-ink border-sidebar-line absolute -right-2 -top-2 rounded-full border px-2 py-0.5 text-xs font-medium">
					{badge}
				</div>
			)}

			<div className="flex gap-3">
				<div className="relative">
					{icon}
					{pulse && (
						<motion.div
							animate={{
								scale: [1, 1.2, 1],
								opacity: [1, 0.5, 1]
							}}
							transition={{duration: 2, repeat: Infinity}}
							className="bg-accent absolute -right-1 -top-1 size-2 rounded-full"
						/>
					)}
				</div>
				<div className="flex-1">
					<div className="text-ink mb-1 text-3xl font-bold">
						{value}
					</div>
					<div className="text-ink-dull mb-1 text-xs">{label}</div>
					<div className="text-ink-faint text-xs">{subtitle}</div>
				</div>
			</div>
		</div>
	);
}
