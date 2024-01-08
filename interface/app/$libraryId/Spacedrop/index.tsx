import { Planet } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useEffect, useRef, useState } from 'react';
import { proxy } from 'valtio';
import { useBridgeMutation, useDiscoveredPeers, useP2PEvents } from '@sd/client';
import { toast } from '@sd/ui';
import { Icon } from '~/components';
import { expandRect, isWithinRect, useDropzone } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { TOP_BAR_ICON_STYLE } from '../TopBar/TopBarOptions';
import { useIncomingSpacedropToast, useSpacedropProgressToast } from './toast';

// TODO: Do this using React context/state
const hackyState = proxy({
	triggeredByDnd: false
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

	// TODO: When the bounce animation starts it glitches out the logo

	// TODO: Disable the bouncing animation once the modal is open
	return (
		<div ref={ref} className={dndState === 'active' ? 'animate-bounce' : ''}>
			<Planet className={TOP_BAR_ICON_STYLE} />
		</div>
	);
}

export function Spacedrop() {
	const ref = useRef<HTMLDivElement>(null);
	const incomingRequestToast = useIncomingSpacedropToast();
	const progressToast = useSpacedropProgressToast();
	const platform = usePlatform();
	const [isOpen, setIsOpen] = useState(false); // TODO: Handle this

	const wasTriggeredByDnd = hackyState.triggeredByDnd;
	useEffect(() => {
		hackyState.triggeredByDnd = false;
	}, []);

	// TODO: If you use DND to open the window but drag the file out, it should autoclose. If it was manually opened don't do anything different.
	useEffect(() => {
		if (!ref.current) return;
		if (!platform.subscribeToDragAndDropEvents) return;

		console.log('SETUP');

		let finished = false;
		let mouseEnteredPopup = false;
		const rect = expandRect(ref.current.getBoundingClientRect(), 10);

		const unsub = platform.subscribeToDragAndDropEvents((event) => {
			if (finished) return;

			if (event.type === 'Hovered') {
				const isWithinRectNow = isWithinRect(event.x, event.y, rect);

				console.log(
					ref.current,
					rect,
					event.x,
					event.y,
					isWithinRectNow,
					mouseEnteredPopup
				); // TODO: Remove

				if (mouseEnteredPopup) {
					if (!isWithinRectNow) {
						console.log('LEAVE');
						// TODO: Close the popup if `wasTriggeredByDnd`
					}
				} else {
					mouseEnteredPopup = isWithinRectNow;
					if (mouseEnteredPopup) console.log('ENTERED');
				}
			} else if (event.type === 'Dropped') {
				mouseEnteredPopup = false;
			} else if (event.type === 'Cancelled') {
				mouseEnteredPopup = false;
			}
		});

		return () => {
			finished = true;
			void unsub.then((unsub) => unsub());
		};
	}, [platform, ref]);

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

	const discoveredPeers = useDiscoveredPeers();
	const doSpacedrop = useBridgeMutation('p2p.spacedrop');

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
			.then(() => setIsOpen(false));
	};

	return (
		<div ref={ref} className="flex h-full max-w-[300px] flex-col">
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
