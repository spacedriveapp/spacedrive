import Ball from '@sd/assets/icons/Ball.png';
import ComputeIcon from '@sd/assets/icons/Compute.png';
import DevicesIcon from '@sd/assets/icons/Devices.png';
import HDD from '@sd/assets/icons/HDD.png';
import IndexedIcon from '@sd/assets/icons/Indexed.png';
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

function formatBytes(bytes: number): string {
	if (bytes === 0) return '0 B';
	const k = 1024;
	const sizes = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
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

	return (
		<div className="px-8 py-8">
			<div className="grid grid-cols-2 gap-8 lg:grid-cols-4">
				{/* Total Storage */}
				<StatCard
					icon={
						<img
							src={HDD}
							alt="Storage"
							className="size-8 opacity-80"
							// style={{ filter: 'drop-shadow(0 0 4px rgba(217, 70, 239, 0.4))' }}
						/>
					}
					label="Total Storage"
					value={formatBytes(totalStorage)}
					subtitle={
						<>
							<span className="text-accent">
								{formatBytes(usedStorage)}
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
							className="size-8 opacity-80"
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
							src={DevicesIcon}
							alt="Devices"
							className="size-8 opacity-80"
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
							className="size-8 opacity-80"
						/>
					}
					label="AI Compute Power"
					value="70 TOPS"
					subtitle="across all devices"
					color="from-purple-500 to-pink-500"
				/>
			</div>
		</div>
	);
}

interface StatCardProps {
	icon: React.ReactNode;
	label: string;
	value: string | number;
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

			<div className="flex items-start gap-3">
				<div className="relative mt-2">
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

				<div>
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
