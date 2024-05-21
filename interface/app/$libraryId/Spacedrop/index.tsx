import { Planet } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useEffect, useRef, useState } from 'react';
import { proxy } from 'valtio';
import {
	HardwareModel,
	useBridgeMutation,
	useDiscoveredPeers,
	useP2PEvents,
	useSelector
} from '@sd/client';
import { toast } from '@sd/ui';
import { Icon } from '~/components';
import { useDropzone, useLocale, useOnDndLeave } from '~/hooks';
import { hardwareModelToIcon } from '~/util/hardware';
import { usePlatform } from '~/util/Platform';

import { TOP_BAR_ICON_CLASSLIST } from '../TopBar/TopBarOptions';
import { useIncomingSpacedropToast, useSpacedropProgressToast } from './toast';

// TODO: This is super hacky so should probs be rewritten but for now it works.
const hackyState = proxy({
	triggeredByDnd: false,
	openPanels: 0
});

export function SpacedropProvider() {
	const incomingRequestToast = useIncomingSpacedropToast();
	const progressToast = useSpacedropProgressToast();
	const { t } = useLocale();

	useP2PEvents((data) => {
		if (data.type === 'SpacedropRequest') {
			incomingRequestToast(data);
		} else if (data.type === 'SpacedropProgress') {
			progressToast(data);
		} else if (data.type === 'SpacedropRejected') {
			// TODO: Add more information to this like peer name, etc in future
			toast.warning(t('spacedrop_rejected'));
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
			<Planet className={TOP_BAR_ICON_CLASSLIST} />
		</div>
	);
}

export function Spacedrop({ triggerClose }: { triggerClose: () => void }) {
	const ref = useRef<HTMLDivElement>(null);
	const discoveredPeers = useDiscoveredPeers();
	const doSpacedrop = useBridgeMutation('p2p.spacedrop');
	const { t } = useLocale();
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
			toast.warning(t('spacedrop_already_progress'));
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

				<div className="flex flex-col pt-2">
					<p className="text-center text-ink-dull">{t('spacedrop_description')}</p>
					{discoveredPeers.size === 0 && (
						<div
							className={clsx(
								'mt-3 flex items-center justify-center gap-3 rounded-md border border-dashed border-app-line bg-app-darkBox px-3 py-2 font-medium text-ink'
							)}
						>
							<p className="text-center text-ink-faint">{t('no_nodes_found')}</p>
						</div>
					)}
					<div className="flex flex-col space-y-2">
						{Array.from(discoveredPeers).map(([id, meta]) => (
							<Node
								key={id}
								id={id}
								name={meta.metadata.name}
								model={meta.metadata.device_model ?? 'Other'}
								onDropped={onDropped}
							/>
						))}
					</div>
				</div>
			</div>
		</div>
	);
}

function Node({
	id,
	name,
	model,
	onDropped
}: {
	id: string;
	name: string;
	model: HardwareModel;
	onDropped: (id: string, files: string[]) => void;
}) {
	const ref = useRef<HTMLDivElement>(null);
	const platform = usePlatform();

	const state = useDropzone({
		ref,
		onDrop: (files) => onDropped(id, files)
	});

	const { t } = useLocale();

	return (
		<div
			ref={ref}
			className={clsx(
				'flex items-center justify-start gap-2 rounded-md border bg-app-darkBox px-3 py-2 font-medium text-ink',
				state === 'hovered'
					? 'border-solid border-accent-deep'
					: 'border-dashed border-app-line'
			)}
			onClick={() => {
				if (!platform.openFilePickerDialog) {
					toast.warning(t('file_picker_not_supported'));
					return;
				}

				platform.openFilePickerDialog?.().then((file) => {
					const files = Array.isArray(file) || file === null ? file : [file];
					if (files === null || files.length === 0) return;
					onDropped(id, files);
				});
			}}
		>
			<Icon name={hardwareModelToIcon(model)} size={20} />
			<h1>{name}</h1>
		</div>
	);
}
