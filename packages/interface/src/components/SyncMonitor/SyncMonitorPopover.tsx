import {
  ArrowsClockwise,
  ArrowsOut,
  CircleNotch,
  FunnelSimple,
} from "@phosphor-icons/react";
import { Popover, TopBarButton, usePopover } from "@sd/ui";
import clsx from "clsx";
import { motion } from "framer-motion";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { ActivityFeed } from "./components/ActivityFeed";
import { PeerList } from "./components/PeerList";
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
      align="start"
      className="!p-0 !bg-app !rounded-xl z-50 max-h-[520px] w-[380px]"
      popover={popover}
      side="top"
      sideOffset={8}
      trigger={
        <button
          className={clsx(
            "relative flex w-full items-center gap-2 rounded-lg px-2 py-1.5 font-medium text-sm",
            "cursor-default text-sidebar-inkDull",
            className
          )}
        >
          <div className="size-4">
            {isSyncing ? (
              <CircleNotch className="animate-spin" size={16} weight="bold" />
            ) : (
              <ArrowsClockwise size={16} weight="bold" />
            )}
          </div>
          <span>Sync</span>
          {onlinePeerCount > 0 && (
            <span className="flex h-[18px] min-w-[18px] items-center justify-center rounded-full bg-accent px-1 font-bold text-[10px] text-white">
              {onlinePeerCount}
            </span>
          )}
        </button>
      }
    >
      <div className="flex items-center justify-between border-app-line border-b px-4 py-3">
        <h3 className="font-semibold text-ink text-sm">Sync Monitor</h3>

        <div className="flex items-center gap-2">
          {onlinePeerCount > 0 && (
            <span className="text-ink-dull text-xs">
              {onlinePeerCount} {onlinePeerCount === 1 ? "peer" : "peers"}{" "}
              online
            </span>
          )}

          <TopBarButton
            icon={ArrowsOut}
            onClick={() => navigate("/sync")}
            title="Open full sync monitor"
          />

          <TopBarButton
            active={showActivityFeed}
            icon={FunnelSimple}
            onClick={() => setShowActivityFeed(!showActivityFeed)}
            title={showActivityFeed ? "Show peers" : "Show activity feed"}
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
      <div className="border-app-line border-b bg-app-box/50 px-4 py-2">
        <div className="flex items-center gap-2">
          <div
            className={`size-2 rounded-full ${getStateColor(sync.currentState)}`}
          />
          <span className="font-medium text-ink-dull text-xs">
            {sync.currentState}
          </span>
        </div>
      </div>
      <motion.div
        animate={{
          height: showActivityFeed
            ? Math.min(sync.recentActivity.length * 40 + 16, 400)
            : Math.min(sync.peers.length * 80 + 16, 400),
        }}
        className="no-scrollbar overflow-y-auto"
        initial={false}
        transition={{ duration: 0.2, ease: [0.25, 1, 0.5, 1] }}
      >
        {showActivityFeed ? (
          <ActivityFeed activities={sync.recentActivity} />
        ) : (
          <PeerList currentState={sync.currentState} peers={sync.peers} />
        )}
      </motion.div>
    </>
  );
}
