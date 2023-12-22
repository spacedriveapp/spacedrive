import { CloudArrowUp, Planet } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useEffect, useRef, useState } from 'react';
import { proxy } from 'valtio';
import { useP2PEvents } from '@sd/client';
import { dialogManager, toast } from '@sd/ui';
import { Icon } from '~/components';
import { expandRect, isWithinRect, useDropzone } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { TOP_BAR_ICON_STYLE } from '../TopBar/TopBarOptions';
import { SpacedropDialog } from './dialog';
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

	return (
		<div ref={ref} className="flex h-full max-w-[300px] flex-col">
			<div className="flex w-full flex-col items-center p-4">
				<Icon name="Spacedrop" size={56} />
				<span className="text-lg font-bold">Spacedrop</span>

				<div className="flex flex-col space-y-4 pt-2">
					<Node name="Oscar's Generic Android Handset" />
					<Node name="Another Phone" />
				</div>

				{/* <div ref={dropzoneRef} className="mt-3 flex w-full items-center justify-center">
					<label
						className="dark:hover:bg-bray-800 flex h-64 w-full cursor-pointer flex-col items-center justify-center rounded-lg border-2 border-dashed border-gray-300 bg-gray-50 hover:bg-gray-100 dark:border-gray-600 dark:bg-gray-700 dark:hover:border-gray-500 dark:hover:bg-gray-600"
						onClick={() =>
							platform.openFilePickerDialog?.().then((path) => {
								if (path === null) return;
								if (isOpen) return;

								setIsOpen(true);
								dialogManager
									.create((dp) => (
										<SpacedropDialog {...dp} path={castToArray(path)} />
									))
									.then(() => setIsOpen(false));
							})
						}
					>
						<div className="flex flex-col items-center justify-center pb-6 pt-5">
							<CloudArrowUp size={32} className="text-black" />
							<p className="mb-2 text-sm text-gray-500 dark:text-gray-400">
								<span className="font-semibold">Click to upload</span> or drag and
								drop
							</p>
						</div>
					</label>
				</div> */}
			</div>
		</div>
	);
}

function Node({ name }: { name: string }) {
	const ref = useRef<HTMLDivElement>(null);

	const state = useDropzone({
		ref,
		onDrop: (files) => {
			alert('Spacedroping to ' + name + ' ' + files);

			// TODO: Hook up the backend
		}
	});

	return (
		<div
			ref={ref}
			className={clsx('border border-dashed px-4 py-2', state === 'hovered' && 'wiggle')}
		>
			<h1>{name}</h1>
		</div>
	);
}

function castToArray<T>(t: T | T[]) {
	if (Array.isArray(t)) return t;
	return [t];
}
