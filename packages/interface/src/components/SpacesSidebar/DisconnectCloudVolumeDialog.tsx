import { PlugsConnected } from '@phosphor-icons/react';
import { Dialog, dialogManager, useDialog, type UseDialogProps } from '@spacedrive/primitives';
import { useQueryClient } from '@tanstack/react-query';
import { useForm } from 'react-hook-form';
import { useLibraryMutation } from '../../contexts/SpacedriveContext';

interface DisconnectCloudVolumeDialogProps extends UseDialogProps {
	fingerprint: string;
	volumeName: string;
}

/**
 * Opens a confirmation dialog that removes a cloud volume and purges its
 * encrypted credentials.
 *
 * This maps to `volumes.remove_cloud`, which deletes the volume plus the
 * stored OAuth/API credentials. Use this for cloud volumes only; for local
 * volumes, the generic `volumes.untrack` flow is sufficient.
 */
export function useDisconnectCloudVolumeDialog() {
	return (fingerprint: string, volumeName: string) =>
		dialogManager.create((props: UseDialogProps) => (
			<DisconnectCloudVolumeDialog
				{...props}
				fingerprint={fingerprint}
				volumeName={volumeName}
			/>
		));
}

function DisconnectCloudVolumeDialog({
	fingerprint,
	volumeName,
	...props
}: DisconnectCloudVolumeDialogProps) {
	const dialog = useDialog(props);
	const form = useForm();
	const queryClient = useQueryClient();
	const removeCloud = useLibraryMutation('volumes.remove_cloud', {
		onSuccess: () => {
			// Force a refetch so the volume disappears from the sidebar immediately,
			// matching the pattern used by DeleteLocationDialog.
			queryClient.invalidateQueries({
				predicate: (query) => {
					const key = query.queryKey;
					return Array.isArray(key) && key[0] === 'query:volumes.list';
				},
			});
			dialogManager.setState(dialog.id, { open: false });
		},
	});

	const handleDisconnect = async () => {
		try {
			await removeCloud.mutateAsync({ fingerprint });
		} catch (error) {
			console.error('Failed to disconnect cloud volume:', error);
		}
	};

	return (
		<Dialog
			dialog={dialog}
			form={form}
			title="Disconnect Cloud Volume"
			description={`Disconnect "${volumeName}"? The stored credentials will be permanently removed. You can reconnect later by adding the cloud storage again.`}
			icon={<PlugsConnected className="text-red-400" weight="bold" />}
			ctaLabel="Disconnect"
			ctaDanger
			cancelLabel="Cancel"
			cancelBtn
			onSubmit={form.handleSubmit(handleDisconnect)}
			loading={removeCloud.isPending}
		/>
	);
}
