import { Circle, Lightning } from '@phosphor-icons/react';
import clsx from 'clsx';
import { formatDistanceToNow } from 'date-fns';
import type { SyncPeerActivity, SyncState } from '../types';

interface PeerListProps {
	peers: SyncPeerActivity[];
	currentState: SyncState;
}

export function PeerList({ peers }: PeerListProps) {
	if (peers.length === 0) {
		return (
			<div className="flex flex-col items-center justify-center py-12 px-4">
				<Circle className="size-8 text-ink-faint mb-2" weight="duotone" />
				<p className="text-sm text-ink-dull text-center">No paired devices</p>
				<p className="text-xs text-ink-faint text-center mt-1">
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
		<div className="bg-app-box rounded-lg p-3 border border-app-line">
			<div className="flex items-start justify-between mb-2">
				<div className="flex items-center gap-2">
					<div
						className={clsx(
							'size-2 rounded-full',
							peer.isOnline ? 'bg-green-500' : 'bg-ink-faint'
						)}
					/>
					<span className="text-sm font-medium text-ink">{peer.deviceName}</span>
				</div>

				{peer.watermarkLagMs && peer.watermarkLagMs > 60000 && (
					<Lightning className="size-4 text-yellow-500" weight="fill" />
				)}
			</div>

			<div className="flex items-center justify-between text-xs text-ink-dull">
				<span>
					{peer.isOnline
						? 'Online'
						: `Last seen ${formatDistanceToNow(new Date(peer.lastSeen), { addSuffix: true })}`}
				</span>
			</div>

			<div className="flex items-center gap-4 mt-2 text-xs text-ink-faint">
				<div className="flex flex-col">
					<span>{formatBytes(peer.bytesReceived)}</span>
					<span className="text-[10px]">received</span>
				</div>
				<div className="flex flex-col">
					<span>{peer.entriesReceived.toLocaleString()}</span>
					<span className="text-[10px]">changes</span>
				</div>
			</div>
		</div>
	);
}
