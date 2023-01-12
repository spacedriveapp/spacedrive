import { useLibraryMutation } from '@sd/client';
import { Dialog } from '@sd/ui';
import { useState } from 'react';

import Slider from '../primitive/Slider';

import { CheckBox, Input, useZodForm, z } from '@sd/ui/src/forms';

// these props are all shared
interface EraseDialogProps {
	open: boolean;
	setOpen: (isShowing: boolean) => void;
	location_id: number | null;
	path_id: number | undefined;
}

const schema = z.object({
	// outputPath: z.string()
	passes: z.number()
});

export const EraseFileDialog = (props: EraseDialogProps) => {
	const eraseFile = useLibraryMutation('files.eraseFiles');

	const form = useZodForm({
		schema
	});

	const onSubmit = form.handleSubmit((data) => {
		props.setOpen(false);

		props.location_id &&
			props.path_id &&
			eraseFile.mutate({
				// algorithm: data.encryptionAlgo as Algorithm,
				// key_uuid: data.key,
				location_id: props.location_id,
				path_id: props.path_id,
				passes: data.passes
				// metadata: data.metadata,
				// preview_media: data.previewMedia,
				// output_path: data.outputPath || null
			});

		form.reset();
	});

	const [passes, setPasses] = useState([4]);

	return (
		<Dialog
			form={form}
			onSubmit={onSubmit}
			open={props.open}
			setOpen={props.setOpen}
			title="Erase a file"
			description="Configure your erasure settings."
			loading={eraseFile.isLoading}
			ctaLabel="Erase"
		>
			<div className="flex flex-col mt-2">
				<span className="text-xs font-bold"># of passes</span>

				<div className="flex flex-row space-x-2">
					<div className="relative flex flex-grow mt-2">
						<Slider
							value={passes}
							max={16}
							min={1}
							step={1}
							defaultValue={[64]}
							onValueChange={(e) => {
								setPasses(e);
							}}
						/>
					</div>
					<span className="text-sm mt-2.5 font-medium">{passes}</span>
				</div>
			</div>
		</Dialog>
	);
};
