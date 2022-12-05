import { useLibraryMutation } from '@sd/client';
import { Button, Dialog } from '@sd/ui';
import { save } from '@tauri-apps/api/dialog';
import { useState } from 'react';

interface DecryptDialogProps {
	open: boolean;
	setOpen: (isShowing: boolean) => void;
	location_id: number | null;
	object_id: number | null;
	setShowAlertDialog: (isShowing: boolean) => void;
	setAlertDialogData: (data: { title: string; text: string }) => void;
}

export const DecryptFileDialog = (props: DecryptDialogProps) => {
	const { location_id, object_id } = props;
	const decryptFile = useLibraryMutation('files.decryptFiles');
	const [outputPath, setOutputpath] = useState('');

	return (
		<>
			<Dialog
				open={props.open}
				setOpen={props.setOpen}
				title="Decrypt a file"
				description="Leave the output file blank for the default."
				loading={decryptFile.isLoading}
				ctaLabel="Decrypt"
				ctaAction={() => {
					const output = outputPath !== '' ? outputPath : undefined; // need to add functionality for this in rust
					props.setOpen(false);

					location_id &&
						object_id &&
						decryptFile.mutate(
							{
								location_id,
								object_id
							},
							{
								onSuccess: () => {
									props.setAlertDialogData({
										title: 'Info',
										text: 'The decryption job has started successfully. You may track the progress in the job overview panel.'
									});
								},
								onError: () => {
									props.setAlertDialogData({
										title: 'Error',
										text: 'The decryption job failed to start.'
									});
								}
							}
						);

					props.setShowAlertDialog(true);
				}}
			>
				<div className="grid w-full grid-cols-2 gap-4 mt-4 mb-3">
					<div className="flex flex-col">
						<span className="text-xs font-bold">Output file</span>

						<Button
							size="sm"
							variant={outputPath !== '' ? 'accent' : 'gray'}
							className="h-[23px] mt-2"
							onClick={() => {
								// not platform-safe, probably will break on web but `platform` doesn't have a save dialog option
								save()?.then((result) => {
									if (result) setOutputpath(result as string);
								});
							}}
						>
							Select
						</Button>
					</div>
				</div>
			</Dialog>
		</>
	);
};
