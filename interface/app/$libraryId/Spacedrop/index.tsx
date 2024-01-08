import { Planet } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useEffect, useRef, useState } from 'react';
import { proxy } from 'valtio';
import { useBridgeMutation, useDiscoveredPeers, useP2PEvents, useSelector } from '@sd/client';
import { toast } from '@sd/ui';
import { Icon } from '~/components';
import { useDropzone, useOnDndLeave } from '~/hooks';

import { TOP_BAR_ICON_STYLE } from '../TopBar/TopBarOptions';
import { useIncomingSpacedropToast, useSpacedropProgressToast } from './toast';

// TODO: Do this using React context/state
const hackyState = proxy({
	triggeredByDnd: false,
	openPanels: 0
});

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

// This parent component takes care of the hacky stuff. All the proper logic in within `SpacedropChild`
export function Spacedrop() {
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

	return <SpacedropChild wasTriggeredByDnd={wasTriggeredByDnd} />;
}

function SpacedropChild({ wasTriggeredByDnd }: { wasTriggeredByDnd: boolean }) {
	const ref = useRef<HTMLDivElement>(null);
	const incomingRequestToast = useIncomingSpacedropToast();
	const progressToast = useSpacedropProgressToast();
	const discoveredPeers = useDiscoveredPeers();
	const doSpacedrop = useBridgeMutation('p2p.spacedrop');

	useOnDndLeave({
		ref,
		onLeave: () => {
			console.log('TODO: Close');
		}
	});

	// TODO: Should these be here???
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
			.then(() => {
				// TODO: Close the window
				// setIsOpen(false);
			});
	};

	return (
		<div ref={ref} className="flex h-full max-w-[300px] flex-col bg-red-500">
			<div className="flex w-full flex-col items-center p-4">
				<Icon name="Spacedrop" size={56} />
				<span className="text-lg font-bold">Spacedrop</span>

				<div className="flex flex-col space-y-4 pt-2">
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

	const state = useDropzone({
		ref,
		onDrop: (files) => onDropped(id, files)
	});

	// TODO: onClick open a file selector (this should allow us to support web)

	return (
		<div
			ref={ref}
			className={clsx(
				'border px-4 py-2',
				state === 'hovered' ? 'border-solid' : 'border-dashed'
			)}
		>
			<h1>{name}</h1>
		</div>
	);
}
