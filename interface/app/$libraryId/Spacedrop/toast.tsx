import { useEffect, useRef } from 'react';
import { P2PEvent, useBridgeMutation, useSpacedropProgress } from '@sd/client';
import { Input, ProgressBar, toast, ToastId } from '@sd/ui';
import { usePlatform } from '~/util/Platform';

const placeholder = '/Users/oscar/Desktop/demo.txt';

export function useIncomingSpacedropToast() {
	const platform = usePlatform();
	const acceptSpacedrop = useBridgeMutation('p2p.acceptSpacedrop');
	const filePathInput = useRef<HTMLInputElement>(null);

	return (data: Extract<P2PEvent, { type: 'SpacedropRequest' }>) =>
		toast.info(
			{
				title: 'Incoming Spacedrop',
				// TODO: Make this pretty
				body: (
					<>
						<p>
							File '{data.files[0]}' from '{data.peer_name}'
						</p>
						{/* TODO: This will be removed in the future for now it's just a hack */}
						{platform.saveFilePickerDialog ? null : (
							<Input
								ref={filePathInput}
								name="file_path"
								size="sm"
								placeholder={placeholder}
								className="w-full"
							/>
						)}
						{/* TODO: Button to expand the toast and show the entire PeerID for manual verification? */}
					</>
				)
			},
			{
				duration: 30 * 1000,
				onClose: ({ event }) => {
					event !== 'on-action' && acceptSpacedrop.mutate([data.id, null]);
				},
				action: {
					label: 'Accept',
					async onClick() {
						let destinationFilePath = filePathInput.current?.value ?? placeholder;

						if (data.files.length != 1) {
							if (platform.openDirectoryPickerDialog) {
								const result = await platform.openDirectoryPickerDialog({
									title: 'Save Spacedrop',
									multiple: false
								});
								if (!result) {
									return;
								}
								destinationFilePath = result;
							}
						} else {
							if (platform.saveFilePickerDialog) {
								const result = await platform.saveFilePickerDialog({
									title: 'Save Spacedrop',
									defaultPath: data.files?.[0]
								});
								if (!result) {
									return;
								}
								destinationFilePath = result;
							}
						}

						if (destinationFilePath === '') return;
						await acceptSpacedrop.mutateAsync([data.id, destinationFilePath]);
					}
				},
				cancel: 'Reject'
			}
		);
}

export function SpacedropProgress({ toastId, dropId }: { toastId: ToastId; dropId: string }) {
	const progress = useSpacedropProgress(dropId);

	useEffect(() => {
		if (progress === 100) {
			setTimeout(() => toast.dismiss(toastId), 750);
		}
	}, [progress, toastId]);

	return (
		<div className="pt-1">
			<ProgressBar percent={progress ?? 0} />
		</div>
	);
}

export function useSpacedropProgressToast() {
	const cancelSpacedrop = useBridgeMutation(['p2p.cancelSpacedrop']);

	return (data: Extract<P2PEvent, { type: 'SpacedropProgress' }>) => {
		toast.info(
			(id) => ({
				title: 'Spacedrop',
				body: <SpacedropProgress toastId={id} dropId={data.id} />
			}),
			{
				id: data.id,
				duration: Infinity,
				cancel: {
					label: 'Cancel',
					onClick() {
						cancelSpacedrop.mutate(data.id);
					}
				}
			}
		);
	};
}
