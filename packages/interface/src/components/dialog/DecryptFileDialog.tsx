import { useLibraryMutation } from '@sd/client';
import { Button, Dialog } from '@sd/ui';
import { save } from '@tauri-apps/api/dialog';
import { useState } from 'react';

import { GenericAlertDialogProps } from './AlertDialog';

interface DecryptDialogProps {
	open: boolean;
	setOpen: (isShowing: boolean) => void;
	location_id: number | null;
	object_id: number | null;
	setAlertDialogData: (data: GenericAlertDialogProps) => void;
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
					const output = outputPath !== '' ? outputPath : null;
					props.setOpen(false);

					location_id &&
						object_id &&
						decryptFile.mutate(
							{
								location_id,
								object_id,
								output_path: output
							},
							{
								onSuccess: () => {
									props.setAlertDialogData({
										open: true,
										title: 'Info',
										value:
											'The decryption job has started successfully. You may track the progress in the job overview panel.',
										inputBox: false,
										description: ''
									});
								},
								onError: () => {
									props.setAlertDialogData({
										open: true,
										title: 'Error',
										value: 'The decryption job failed to start.',
										inputBox: false,
										description: ''
									});
								}
							}
						);
				}}
			>
				<div className="grid w-full grid-cols-2 gap-4 mt-4 mb-3">
					<div className="flex flex-col">
						<span className="text-xs font-bold">Output file</span>

						<Button
							size="sm"
							variant={outputPath !== '' ? 'accent' : 'gray'}
							className="h-[23px] text-xs leading-3 mt-2"
							type="button"
							onClick={() => {
								// if we allow the user to encrypt multiple files simultaneously, this should become a directory instead
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
