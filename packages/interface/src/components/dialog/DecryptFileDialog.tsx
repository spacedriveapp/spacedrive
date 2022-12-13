import { useLibraryMutation } from '@sd/client';
import { Button, Dialog } from '@sd/ui';
import { useState } from 'react';

import { usePlatform } from '../../util/Platform';
import { GenericAlertDialogProps } from './AlertDialog';

interface DecryptDialogProps {
	open: boolean;
	setOpen: (isShowing: boolean) => void;
	location_id: number | null;
	object_id: number | null;
	setAlertDialogData: (data: GenericAlertDialogProps) => void;
}

export const DecryptFileDialog = (props: DecryptDialogProps) => {
	const platform = usePlatform();
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
								if (!platform.saveFilePickerDialog) {
									// TODO: Support opening locations on web
									props.setAlertDialogData({
										open: true,
										title: 'Error',
										description: '',
										value: "System dialogs aren't supported on this platform.",
										inputBox: false
									});
									return;
								}
								platform.saveFilePickerDialog().then((result) => {
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
