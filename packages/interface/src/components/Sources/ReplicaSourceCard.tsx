import {HardDrive} from '@phosphor-icons/react';
import {useNavigate} from 'react-router-dom';
import type {SourceInfo} from '@sd/ts-client';
import {formatBytes} from '../../routes/explorer/utils';

/**
 * A paired device's source, replicated through the peer-mount plane — same
 * card language as local drives, labelled with the owning device. Opens the
 * explorer routed through that device's slug, which is what serves the
 * listing from the replicated index.
 */

function syncedLabel(iso: string): string {
	const elapsed = Math.max(0, Date.now() - new Date(iso).getTime());
	const minutes = Math.floor(elapsed / 60000);
	if (minutes < 1) return 'synced just now';
	if (minutes < 60) return `synced ${minutes}m ago`;
	const hours = Math.floor(minutes / 60);
	if (hours < 24) return `synced ${hours}h ago`;
	return `synced ${Math.floor(hours / 24)}d ago`;
}

export function ReplicaSourceCard({
	source,
	deviceSlug
}: {
	source: SourceInfo;
	deviceSlug: string | undefined;
}) {
	const navigate = useNavigate();

	const open = () => {
		if (!deviceSlug || !source.root) return;
		const sdPath = {
			Physical: {device_slug: deviceSlug, path: source.root}
		};
		navigate(`/explorer?path=${encodeURIComponent(JSON.stringify(sdPath))}`);
	};

	return (
		<button
			onClick={open}
			title={source.root ?? source.name}
			className="border-app-line bg-app-box hover:border-app-line/80 hover:bg-app-hover group relative rounded-lg border p-4 text-left transition-all"
		>
			<div className="mb-3 flex items-center gap-3">
				<div className="bg-app-selected/40 flex size-9 shrink-0 items-center justify-center rounded-lg">
					<HardDrive size={20} weight="bold" className="text-ink-dull" />
				</div>
				<div className="min-w-0 flex-1">
					<h3 className="text-ink truncate text-sm font-medium">
						{source.name}
					</h3>
					<p className="text-ink-faint text-xs">
						{source.device_label ?? 'paired device'}
					</p>
				</div>
				<span className="bg-app-selected/40 text-ink-faint rounded-full px-2 py-0.5 text-[10px] font-medium">
					replica
				</span>
			</div>

			<div className="text-ink-faint flex items-center justify-between text-xs">
				<span>
					{source.item_count.toLocaleString()} items
					{source.total_bytes != null &&
						` · ${formatBytes(source.total_bytes)}`}
				</span>
				<span>
					{source.last_synced
						? syncedLabel(source.last_synced)
						: 'never synced'}
				</span>
			</div>
		</button>
	);
}
