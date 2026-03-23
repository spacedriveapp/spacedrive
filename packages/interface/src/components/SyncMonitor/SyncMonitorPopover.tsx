import {
	ArrowsClockwise,
	CircleNotch,
	ArrowsOut,
	FunnelSimple,
} from "@phosphor-icons/react";
import { Popover, usePopover, TopBarButton } from "@sd/ui";
import clsx from "clsx";
import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { shouldNavigate } from "../../util/navigation";
import { motion } from "framer-motion";
import { PeerList } from "./components/PeerList";
import { ActivityFeed } from "./components/ActivityFeed";
import { useSyncCount } from "./hooks/useSyncCount";
import { useSyncMonitor } from "./hooks/useSyncMonitor";

interface SyncMonitorPopoverProps {
	className?: string;
}

export function SyncMonitorPopover({ className }: SyncMonitorPopoverProps) {
	const navigate = useNavigate();
	const popover = usePopover();
	const [showActivityFeed, setShowActivityFeed] = useState(false);

	const { onlinePeerCount, isSyncing } = useSyncCount();

	useEffect(() => {
		if (popover.open) {
			setShowActivityFeed(false);
		}
	}, [popover.open]);

	return (
		<Popover
			popover={popover}
			trigger={
				<button
					className={clsx(
						"w-full relative flex items-center gap-2 rounded-lg px-2 py-1.5 text-sm font-medium",
						"text-sidebar-inkDull cursor-default",
						className,
					)}
				>
					<div className="size-4">
						{isSyncing ? (
							<CircleNotch
								className="animate-spin"
								weight="bold"
								size={16}
							/>
						) : (
							<ArrowsClockwise weight="bold" size={16} />
						)}
					</div>
					<span>Sync</span>
					{onlinePeerCount > 0 && (
						<span className="flex items-center justify-center min-w-[18px] h-[18px] px-1 text-[10px] font-bold text-white bg-accent rounded-full">
							{onlinePeerCount}
						</span>
					)}
				</button>
			}
			side="top"
			align="start"
			sideOffset={8}
			className="w-[380px] max-h-[520px] z-50 !p-0 !bg-app !rounded-xl"
		>
			<div className="flex items-center justify-between px-4 py-3 border-b border-app-line">
				<h3 className="text-sm font-semibold text-ink">Sync Monitor</h3>

				<div className="flex items-center gap-2">
					{onlinePeerCount > 0 && (
						<span className="text-xs text-ink-dull">
							{onlinePeerCount}{" "}
							{onlinePeerCount === 1 ? "peer" : "peers"} online
						</span>
					)}

					<TopBarButton
						icon={ArrowsOut}
						onClick={(e: React.MouseEvent) => { if (!shouldNavigate(e)) return; navigate("/sync"); }}
						title="Open full sync monitor"
					/>

					<TopBarButton
						icon={FunnelSimple}
						active={showActivityFeed}
						onClick={() => setShowActivityFeed(!showActivityFeed)}
						title={
							showActivityFeed
								? "Show peers"
								: "Show activity feed"
						}
					/>
				</div>
			</div>

			{popover.open && (
				<SyncMonitorContent showActivityFeed={showActivityFeed} />
			)}
		</Popover>
	);
}

function SyncMonitorContent({
	showActivityFeed,
}: {
	showActivityFeed: boolean;
}) {
	const sync = useSyncMonitor();

	const getStateColor = (state: string) => {
		switch (state) {
			case "Ready":
				return "bg-green-500";
			case "Backfilling":
				return "bg-yellow-500";
			case "CatchingUp":
				return "bg-accent";
			case "Uninitialized":
				return "bg-ink-faint";
			case "Paused":
				return "bg-ink-dull";
			default:
				return "bg-ink-faint";
		}
	};

	return (
		<>
			<div className="px-4 py-2 border-b border-app-line bg-app-box/50">
				<div className="flex items-center gap-2">
					<div
						className={`size-2 rounded-full ${getStateColor(sync.currentState)}`}
					/>
					<span className="text-xs font-medium text-ink-dull">
						{sync.currentState}
					</span>
				</div>
			</div>
			<motion.div
				className="overflow-y-auto no-scrollbar"
				initial={false}
				animate={{
					height: showActivityFeed
						? Math.min(sync.recentActivity.length * 40 + 16, 400)
						: Math.min(sync.peers.length * 80 + 16, 400),
				}}
				transition={{ duration: 0.2, ease: [0.25, 1, 0.5, 1] }}
			>
				{showActivityFeed ? (
					<ActivityFeed activities={sync.recentActivity} />
				) : (
					<PeerList
						peers={sync.peers}
						currentState={sync.currentState}
					/>
				)}
			</motion.div>
		</>
	);
}
