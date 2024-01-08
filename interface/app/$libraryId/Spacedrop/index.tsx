import { Planet } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useEffect, useRef, useState } from 'react';
import { proxy } from 'valtio';
import { useBridgeMutation, useDiscoveredPeers, useP2PEvents, useSelector } from '@sd/client';
import { toast } from '@sd/ui';
import { Icon } from '~/components';
import { useDropzone, useOnDndLeave } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { TOP_BAR_ICON_STYLE } from '../TopBar/TopBarOptions';
import { useIncomingSpacedropToast, useSpacedropProgressToast } from './toast';

// TODO: This is super hacky so should probs be rewritten but for now it works.
const hackyState = proxy({
	triggeredByDnd: false,
	openPanels: 0
});

export function SpacedropProvider() {
	const incomingRequestToast = useIncomingSpacedropToast();
	const progressToast = useSpacedropProgressToast();

	useP2PEvents((data) => {
		if (data.type === 'SpacedropRequest') {
			incomingRequestToast(data);
		} else if (data.type === 'SpacedropProgress') {
			progressToast(data);
		} else if (data.type === 'SpacedropRejected') {
			// TODO: Add more information to this like peer name, etc in future
			toast.warning('Spacedrop Rejected');
		}
	});

	return null;
}

export function SpacedropButton({ triggerOpen }: { triggerOpen: () => void }) {
	const ref = useRef<HTMLDivElement>(null);
	const dndState = useDropzone({
		ref,
		onHover: () => {
			hackyState.triggeredByDnd = true;
			triggerOpen();
		},
		extendBoundsBy: 10
	});
	const isPanelOpen = useSelector(hackyState, (s) => s.openPanels > 0);

	return (
		<div ref={ref} className={dndState === 'active' && !isPanelOpen ? 'animate-bounce' : ''}>
			<Planet className={TOP_BAR_ICON_STYLE} />
		</div>
	);
}

export function Spacedrop({ triggerClose }: { triggerClose: () => void }) {
	const ref = useRef<HTMLDivElement>(null);
	const discoveredPeers = useDiscoveredPeers();
	const doSpacedrop = useBridgeMutation('p2p.spacedrop');

	// We keep track of how many instances of this component is rendering.
	// This is used by `SpacedropButton` to determine if the animation should stop.
	useEffect(() => {
		hackyState.openPanels += 1;
		return () => {
			hackyState.openPanels -= 1;
		};
	});

	// This is intentionally not reactive.
	// We only want the value at the time of the initial render.
	// Then we reset it to false.
	const [wasTriggeredByDnd] = useState(() => hackyState.triggeredByDnd);
	useEffect(() => {
		hackyState.triggeredByDnd = false;
	}, []);

	useOnDndLeave({
		ref,
		onLeave: () => {
			if (wasTriggeredByDnd) triggerClose();
		},
		extendBoundsBy: 30
	});

	const onDropped = (id: string, files: string[]) => {
		if (doSpacedrop.isLoading) {
			toast.warning('Spacedrop already in progress');
			return;
		}

		doSpacedrop
			.mutateAsync({
				identity: id,
				file_path: files
			})
			.then(() => triggerClose());
	};

	return (
		<div ref={ref} className="flex h-full max-w-[300px] flex-col">
			<div className="flex w-full flex-col items-center p-4">
				<Icon name="Spacedrop" size={56} />
				<span className="text-lg font-bold">Spacedrop</span>

				<div className="flex flex-col space-y-4 pt-2">
					{discoveredPeers.size === 0 && (
						<div className="flex flex-col text-center">
							<span className="text-sm text-gray-400">
								No Spacedrive nodes were
								<br /> found on your network
							</span>
						</div>
					)}
					{Array.from(discoveredPeers).map(([id, meta]) => (
						<Node key={id} id={id} name={meta.name} onDropped={onDropped} />
					))}
				</div>
			</div>
		</div>
	);
}

function Node({
	id,
	name,
	onDropped
}: {
	id: string;
	name: string;
	onDropped: (id: string, files: string[]) => void;
}) {
	const ref = useRef<HTMLDivElement>(null);
	const platform = usePlatform();

	const state = useDropzone({
		ref,
		onDrop: (files) => onDropped(id, files)
	});

	return (
		<div
			ref={ref}
			className={clsx(
				'border px-4 py-2',
				state === 'hovered' ? 'border-solid' : 'border-dashed'
			)}
			onClick={() => {
				if (!platform.openFilePickerDialog) {
					toast.warning('File picker not supported on this platform');
					return;
				}

				platform.openFilePickerDialog?.().then((file) => {
					const files = Array.isArray(file) || file === null ? file : [file];
					if (files === null || files.length === 0) return;
					onDropped(id, files);
				});
			}}
		>
			<h1>{name}</h1>
		</div>
	);
}
