import { useState } from 'react';
import { useLibraryMutation } from '@sd/client';
import { Dialog, UseDialogProps, useDialog } from '@sd/ui';
import { useZodForm, z } from '@sd/ui/src/forms';
import Slider from '../primitive/Slider';

interface EraseDialogProps extends UseDialogProps {
	location_id: number;
	path_id: number;
}

const schema = z.object({
	passes: z.number()
});

export const EraseFileDialog = (props: EraseDialogProps) => {
	const dialog = useDialog(props);

	const eraseFile = useLibraryMutation('files.eraseFiles');

	const form = useZodForm({
		schema,
		defaultValues: {
			passes: 4
		}
	});

	const onSubmit = form.handleSubmit((data) =>
		eraseFile.mutateAsync({
			location_id: props.location_id,
			path_id: props.path_id,
			passes: data.passes
		})
	);

	const [passes, setPasses] = useState([4]);

	return (
		<Dialog
			form={form}
			onSubmit={onSubmit}
			dialog={dialog}
			title="Erase a file"
			description="Configure your erasure settings."
			loading={eraseFile.isLoading}
			ctaLabel="Erase"
		>
			<div className="mt-2 flex flex-col">
				<span className="text-xs font-bold"># of passes</span>

				<div className="flex flex-row space-x-2">
					<div className="relative mt-2 flex flex-grow">
						<Slider
							value={passes}
							max={16}
							min={1}
							step={1}
							defaultValue={[4]}
							onValueChange={(e) => {
								setPasses(e);
								form.setValue('passes', e[0]);
							}}
						/>
					</div>
					<span className="mt-2.5 text-sm font-medium">{passes}</span>
				</div>
			</div>

			<p>TODO: checkbox for "erase all matching files" (only if a file is selected)</p>
		</Dialog>
	);
};
