import {
  ArrowDown,
  ArrowsClockwise,
  CheckCircle,
  Circle,
  PlugsConnected,
  Warning,
} from "@phosphor-icons/react";
import clsx from "clsx";
import type { SyncActivity } from "../types";
import { timeAgo } from "../utils";

interface ActivityFeedProps {
  activities: SyncActivity[];
}

export function ActivityFeed({ activities }: ActivityFeedProps) {
  if (activities.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center px-4 py-12">
        <Circle className="mb-2 size-8 text-ink-faint" weight="duotone" />
        <p className="text-center text-ink-dull text-sm">No recent activity</p>
        <p className="mt-1 text-center text-ink-faint text-xs">
          Activity will appear here when syncing
        </p>
      </div>
    );
  }

  return (
    <div className="flex flex-col p-2">
      {activities.map((activity, index) => (
        <ActivityItem
          activity={activity}
          key={`${activity.timestamp}-${index}`}
        />
      ))}
    </div>
  );
}

function ActivityItem({ activity }: { activity: SyncActivity }) {
  const getIcon = () => {
    switch (activity.eventType) {
      case "broadcast":
        return <ArrowsClockwise className="size-4" weight="bold" />;
      case "received":
        return <ArrowDown className="size-4" weight="bold" />;
      case "applied":
        return <CheckCircle className="size-4" weight="bold" />;
      case "backfill":
        return <ArrowsClockwise className="size-4" weight="bold" />;
      case "connection":
        return <PlugsConnected className="size-4" weight="bold" />;
      case "error":
        return <Warning className="size-4" weight="bold" />;
      default:
        return <Circle className="size-4" weight="bold" />;
    }
  };

  const getIconColor = () => {
    switch (activity.eventType) {
      case "broadcast":
        return "text-accent";
      case "received":
        return "text-accent";
      case "applied":
        return "text-green-500";
      case "backfill":
        return "text-purple-500";
      case "connection":
        return "text-ink-dull";
      case "error":
        return "text-red-500";
      default:
        return "text-ink-faint";
    }
  };

  return (
    <div className="flex items-start gap-3 rounded-md px-2 py-2 transition-colors hover:bg-app-hover">
      <div className={clsx("mt-0.5", getIconColor())}>{getIcon()}</div>
      <div className="min-w-0 flex-1">
        <p className="truncate text-ink text-sm">{activity.description}</p>
        <p className="text-ink-faint text-xs">{timeAgo(activity.timestamp)}</p>
      </div>
    </div>
  );
}
