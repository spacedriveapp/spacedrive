import { Circle, Lightning } from "@phosphor-icons/react";
import clsx from "clsx";
import type { SyncPeerActivity, SyncState } from "../types";
import { timeAgo } from "../utils";

interface PeerListProps {
  peers: SyncPeerActivity[];
  currentState: SyncState;
}

export function PeerList({ peers }: PeerListProps) {
  if (peers.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center px-4 py-12">
        <Circle className="mb-2 size-8 text-ink-faint" weight="duotone" />
        <p className="text-center text-ink-dull text-sm">No paired devices</p>
        <p className="mt-1 text-center text-ink-faint text-xs">
          Pair a device to start syncing
        </p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-2 p-2">
      {peers.map((peer) => (
        <PeerCard key={peer.deviceId} peer={peer} />
      ))}
    </div>
  );
}

function PeerCard({ peer }: { peer: SyncPeerActivity }) {
  const formatBytes = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  return (
    <div className="rounded-lg border border-app-line bg-app-box p-3">
      <div className="mb-2 flex items-start justify-between">
        <div className="flex items-center gap-2">
          <div
            className={clsx(
              "size-2 rounded-full",
              peer.isOnline ? "bg-green-500" : "bg-ink-faint"
            )}
          />
          <span className="font-medium text-ink text-sm">
            {peer.deviceName}
          </span>
        </div>

        {peer.watermarkLagMs && peer.watermarkLagMs > 60_000 && (
          <Lightning className="size-4 text-yellow-500" weight="fill" />
        )}
        <span>
          {peer.isOnline ? "Online" : `Last seen ${timeAgo(peer.lastSeen)}`}
        </span>
      </div>

      <div className="mt-2 flex items-center gap-2 text-ink-faint text-xs">
        <div className="flex flex-row gap-2">
          <span>{formatBytes(peer.bytesReceived)}</span>
          <span className="text-[10px]">received</span>
        </div>
        <div className="flex flex-row gap-2">
          <span>{peer.entriesReceived.toLocaleString()}</span>
          <span className="text-[10px]">changes</span>
        </div>
      </div>
    </div>
  );
}
