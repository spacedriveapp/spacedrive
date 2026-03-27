import {CaretLeft, CaretRight, Lightning} from '@phosphor-icons/react';
import ComputeIcon from '@sd/assets/icons/Compute.png';
import DatabaseIcon from '@sd/assets/icons/Database.png';
import DevicesIcon from '@sd/assets/icons/Devices.png';
import IndexedIcon from '@sd/assets/icons/Indexed.png';
import LocationIcon from '@sd/assets/icons/Location.png';
import MobileIcon from '@sd/assets/icons/Mobile.png';
import StorageIcon from '@sd/assets/icons/Storage.png';
import TagsIcon from '@sd/assets/icons/Tags.png';
import {TopBarButton} from '@spaceui/primitives';
import {motion} from 'framer-motion';
import {useEffect, useRef, useState} from 'react';

interface HeroStatsProps {
	totalStorage: number; // bytes
	usedStorage: number; // bytes
	totalFiles: number;
	locationCount: number;
	tagCount: number;
	deviceCount: number;
	uniqueContentCount: number;
	databaseSize: number; // bytes
	sidecarCount: number;
	sidecarSize: number; // bytes
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
	tagCount,
	deviceCount,
	uniqueContentCount,
	databaseSize,
	sidecarCount,
	sidecarSize
}: HeroStatsProps) {
	const scrollRef = useRef<HTMLDivElement>(null);
	const [canScrollLeft, setCanScrollLeft] = useState(false);
	const [canScrollRight, setCanScrollRight] = useState(false);

	const usagePercent =
		totalStorage > 0 ? (usedStorage / totalStorage) * 100 : 0;

	const storageFormatted = formatBytes(totalStorage);
	const usedFormatted = formatBytes(usedStorage);
	const databaseFormatted = formatBytes(databaseSize);
	const sidecarFormatted = formatBytes(sidecarSize);
	const topsValue = 70;
	const topsRank = getTOPSRank(topsValue);

	const updateScrollState = () => {
		if (!scrollRef.current) return;
		const {scrollLeft, scrollWidth, clientWidth} = scrollRef.current;
		setCanScrollLeft(scrollLeft > 0);
		setCanScrollRight(scrollLeft < scrollWidth - clientWidth - 1);
	};

	useEffect(() => {
		updateScrollState();
		window.addEventListener('resize', updateScrollState);
		return () => window.removeEventListener('resize', updateScrollState);
	}, []);

	const scroll = (direction: 'left' | 'right') => {
		if (!scrollRef.current) return;
		// Scroll 4 cards at a time (280px per card + 32px gap)
		const cardWidth = 280;
		const gap = 32;
		const cardsPerPage = 4;
		const scrollAmount = (cardWidth + gap) * cardsPerPage;
		scrollRef.current.scrollBy({
			left: direction === 'left' ? -scrollAmount : scrollAmount,
			behavior: 'smooth'
		});
	};

	return (
		<div className="px-8 py-8">
			<div className="relative">
				{/* Left fade and button */}
				{canScrollLeft && (
					<>
						<div className="pointer-events-none absolute bottom-0 left-0 top-0 z-10 w-24 bg-gradient-to-r from-app to-transparent" />
						<div className="absolute left-2 top-1/2 z-20 -translate-y-1/2">
							<TopBarButton
								icon={CaretLeft}
								onClick={() => scroll('left')}
							/>
						</div>
					</>
				)}

				{/* Scrollable container */}
				<div
					ref={scrollRef}
					onScroll={updateScrollState}
					className="scrollbar-hide flex gap-8 overflow-x-auto"
					style={{scrollbarWidth: 'none'}}
				>
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

				{/* AI Compute Power */}
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

				{/* Library Size (Database) */}
				<StatCard
					icon={
						<img
							src={DatabaseIcon}
							alt="Library"
							className="size-10 opacity-80"
						/>
					}
					label="Library Size"
					value={
						<>
							{databaseFormatted.value}{' '}
							<span className="text-ink-faint text-xl">
								{databaseFormatted.unit}
							</span>
						</>
					}
					subtitle="database on disk"
					color="from-blue-500 to-cyan-500"
				/>

				{/* Sidecar Storage */}
				<StatCard
					icon={
						<img
							src={StorageIcon}
							alt="Sidecars"
							className="size-10 opacity-80"
						/>
					}
					label="Sidecar Storage"
					value={
						<>
							{sidecarFormatted.value}{' '}
							<span className="text-ink-faint text-xl">
								{sidecarFormatted.unit}
							</span>
						</>
					}
					subtitle={`${sidecarCount.toLocaleString()} files generated`}
					color="from-orange-500 to-red-500"
				/>

				{/* Locations */}
				<StatCard
					icon={
						<img
							src={LocationIcon}
							alt="Locations"
							className="size-10 opacity-80"
						/>
					}
					label="Locations"
					value={locationCount}
					subtitle="indexed folders"
					color="from-teal-500 to-green-500"
				/>

				{/* Tags */}
				<StatCard
					icon={
						<img
							src={TagsIcon}
							alt="Tags"
							className="size-10 opacity-80"
						/>
					}
					label="Tags"
					value={tagCount}
					subtitle="organization labels"
					color="from-pink-500 to-rose-500"
				/>
			</div>

			{/* Right fade and button */}
			{canScrollRight && (
				<>
					<div className="pointer-events-none absolute bottom-0 right-0 top-0 z-10 w-24 bg-gradient-to-l from-app to-transparent" />
					<div className="absolute right-2 top-1/2 z-20 -translate-y-1/2">
						<TopBarButton
							icon={CaretRight}
							onClick={() => scroll('right')}
						/>
					</div>
				</>
			)}
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
		<div className="relative min-w-[280px] flex-shrink-0">
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
