import { CloudArrowUp, Planet } from '@phosphor-icons/react';
import { useRef, useState } from 'react';
import { useP2PEvents } from '@sd/client';
import { dialogManager, toast } from '@sd/ui';
import { Icon } from '~/components';
import { useDroppedOn } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { TOP_BAR_ICON_STYLE } from '../TopBar/TopBarOptions';
import { SpacedropDialog } from './dialog';
import { useIncomingSpacedropToast, useSpacedropProgressToast } from './toast';

// TODO: This doesn't support web

export function SpacedropButton() {
	const ref = useRef<HTMLDivElement>(null);
	useDroppedOn(ref);

	return (
		<div ref={ref}>
			<Planet className={TOP_BAR_ICON_STYLE} />
		</div>
	);
}

export function Spacedrop() {
	const incomingRequestToast = useIncomingSpacedropToast();
	const progressToast = useSpacedropProgressToast();
	const platform = usePlatform();
	const [isOpen, setIsOpen] = useState(false); // TODO: Handle this

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

	// useEffect(() => {
	// 	const handler = (e: MouseEvent) => {
	// 		console.log(e);
	// 	};

	// 	document.addEventListener('mousemove', handler, false);
	// 	return document.removeEventListener('mousemove', handler);
	// }, []);

	const ref = useRef<HTMLDivElement>(null);
	useDroppedOn(ref);

	// TODO: Drag and drop working onto icon
	// TODO: Drag and drop onto the UI

	return (
		<div className="flex h-full max-w-[300px] flex-col">
			<div className="flex w-full flex-col items-center p-4">
				<Icon name="Spacedrop" size={56} />
				<span className="text-lg font-bold">Spacedrop</span>

				<div ref={ref} className="mt-3 flex w-full items-center justify-center">
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
				</div>
			</div>
		</div>
	);
}

function castToArray<T>(t: T | T[]) {
	if (Array.isArray(t)) return t;
	return [t];
}
