import { useLibraryMutation } from '@sd/client';
import { Dialog } from '@sd/ui';

import { CheckBox, useZodForm, z } from '@sd/ui/src/forms';

// these props are all shared
interface DeleteDialogProps {
	open: boolean;
	setOpen: (isShowing: boolean) => void;
	location_id: number | null;
	path_id: number | undefined;
}

const schema = z.object({
	// outputPath: z.string()
});

export const DeleteFileDialog = (props: DeleteDialogProps) => {
	const deleteFile = useLibraryMutation('files.deleteFiles');

	const form = useZodForm({
		schema
	});

	const onSubmit = form.handleSubmit((data) => {
		props.setOpen(false);

		props.location_id &&
			props.path_id &&
			deleteFile.mutate({
				// algorithm: data.encryptionAlgo as Algorithm,
				// key_uuid: data.key,
				location_id: props.location_id,
				path_id: props.path_id
				// metadata: data.metadata,
				// preview_media: data.previewMedia,
				// output_path: data.outputPath || null
			});

		form.reset();
	});

	return (
		<Dialog
			form={form}
			onSubmit={onSubmit}
			open={props.open}
			setOpen={props.setOpen}
			title="Delete a file"
			description="Configure your deletion settings."
			loading={deleteFile.isLoading}
			ctaLabel="Delete"
		>
			<div className="grid w-full grid-cols-2 gap-4 mt-4 mb-3">
				<div className="flex">
					<span className="text-sm font-bold mr-3 ml-0.5 mt-0.5">X</span>
					{/* <CheckBox {...form.register('metadata')} /> */}
				</div>
				<div className="flex">
					<span className="text-sm font-bold mr-3 ml-0.5 mt-0.5">Y</span>
					{/* <CheckBox {...form.register('previewMedia')} /> */}
				</div>
			</div>
		</Dialog>
	);
};
