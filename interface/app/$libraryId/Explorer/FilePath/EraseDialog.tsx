// import { useState } from 'react';
// import { FilePath, useLibraryMutation, useZodForm } from '@sd/client';
// import { Dialog, Slider, useDialog, UseDialogProps, z } from '@sd/ui';
// import { useLocale } from '~/hooks';

// interface Props extends UseDialogProps {
// 	locationId: number;
// 	filePaths: FilePath[];
// }

// const schema = z.object({
// 	passes: z.number()
// });

// export default (props: Props) => {
// 	const { t } = useLocale();
// 	const eraseFile = useLibraryMutation('files.eraseFiles');

// 	const form = useZodForm({
// 		schema,
// 		defaultValues: {
// 			passes: 4
// 		}
// 	});

// 	const [passes, setPasses] = useState([4]);

// 	return (
// 		<Dialog
// 			form={form}
// 			onSubmit={form.handleSubmit((data) =>
// 				eraseFile.mutateAsync({
// 					location_id: props.locationId,
// 					file_path_ids: props.filePaths.map((p) => p.id),
// 					passes: data.passes.toString()
// 				})
// 			)}
// 			dialog={useDialog(props)}
// 			title={t('erase_a_file')}
// 			description={t('erase_a_file_description')}
// 			loading={eraseFile.isLoading}
// 			ctaLabel={t('erase')}
// 		>
// 			<div className="mt-2 flex flex-col">
// 				<span className="text-xs font-bold">{t('number_of_passes')}</span>

// 				<div className="flex flex-row space-x-2">
// 					<div className="relative mt-2 flex grow">
// 						<Slider
// 							value={passes}
// 							max={16}
// 							min={1}
// 							step={1}
// 							defaultValue={[4]}
// 							onValueChange={(val) => {
// 								setPasses(val);
// 								form.setValue('passes', val[0] ?? 1);
// 							}}
// 						/>
// 					</div>
// 					<span className="mt-2.5 text-sm font-medium">{passes}</span>
// 				</div>
// 			</div>

// 			{/* <p>TODO: checkbox for "erase all matching files" (only if a file is selected)</p> */}
// 		</Dialog>
// 	);
// };
