import { motion } from "framer-motion";
import { CloudArrowUp, HardDrives, Files, Cpu } from "@phosphor-icons/react";

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
	if (bytes === 0) return "0 B";
	const k = 1024;
	const sizes = ["B", "KB", "MB", "GB", "TB", "PB"];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}

export function HeroStats({
	totalStorage,
	usedStorage,
	totalFiles,
	locationCount,
	deviceCount,
	uniqueContentCount,
}: HeroStatsProps) {
	const usagePercent =
		totalStorage > 0 ? (usedStorage / totalStorage) * 100 : 0;

	return (
		<div className="bg-app-box border border-app-line rounded-2xl p-8">
			<div className="grid grid-cols-2 lg:grid-cols-4 gap-8">
				{/* Total Storage */}
				<StatCard
					icon={HardDrives}
					label="Total Storage"
					value={formatBytes(totalStorage)}
					subtitle={
						<>
							<span className="text-accent">
								{formatBytes(usedStorage)}
							</span>{" "}
							used
						</>
					}
					progress={usagePercent}
					color="from-accent to-cyan-500"
				/>

				{/* Files */}
				<StatCard
					icon={Files}
					label="Files Indexed"
					value={totalFiles.toLocaleString()}
					subtitle={`${uniqueContentCount.toLocaleString()} unique files`}
					color="from-purple-500 to-pink-500"
				/>

				{/* Devices */}
				<StatCard
					icon={CloudArrowUp}
					label="Connected Devices"
					value={deviceCount}
					subtitle={`registered in library`}
					color="from-green-500 to-emerald-500"
				/>

				{/* Storage Health - Future feature */}
				<StatCard
					icon={Cpu}
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
	icon: React.ElementType;
	label: string;
	value: string | number;
	subtitle: React.ReactNode;
	progress?: number;
	pulse?: boolean;
	color: string;
	badge?: string;
}

function StatCard({
	icon: Icon,
	label,
	value,
	subtitle,
	progress,
	pulse,
	color,
	badge,
}: StatCardProps) {
	return (
		<div className="relative">
			{badge && (
				<div className="absolute -top-2 -right-2 px-2 py-0.5 bg-sidebar-box text-sidebar-ink text-xs font-medium rounded-full border border-sidebar-line">
					{badge}
				</div>
			)}

			<div className="flex flex-col gap-3">
				<div className="flex items-center gap-2">
					<div className="p-2 rounded-lg bg-sidebar-box">
						<Icon
							className="size-5 text-sidebar-ink"
							weight="duotone"
						/>
					</div>
					{pulse && (
						<motion.div
							animate={{
								scale: [1, 1.2, 1],
								opacity: [1, 0.5, 1],
							}}
							transition={{ duration: 2, repeat: Infinity }}
							className="size-2 rounded-full bg-accent"
						/>
					)}
				</div>

				<div>
					<div className="text-3xl font-bold text-ink mb-1">
						{value}
					</div>
					<div className="text-xs text-ink-dull mb-1">{label}</div>
					<div className="text-xs text-ink-faint">{subtitle}</div>
				</div>
			</div>
		</div>
	);
}
