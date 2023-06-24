import { Trash } from 'phosphor-react';
import { Tag, useLibraryMutation } from '@sd/client';
import { Button, Switch, Tooltip, dialogManager } from '@sd/ui';
import { Form, Input, useZodForm, z } from '@sd/ui/src/forms';
import { ColorPicker } from '~/components';
import { useDebouncedFormWatch } from '~/hooks';
import Setting from '../../Setting';
import DeleteDialog from './DeleteDialog';

const schema = z.object({
	name: z.string().trim().min(1).max(24).nullable(),
	color: z
		.string()
		.trim()
		.min(2)
		.max(7)
		.regex(/^#([0-9A-F]{1}){1,6}$/i, 'Invalid hex color')
		.nullable()
});

interface Props {
	tag: Tag;
	onDelete: () => void;
}

export default ({ tag, onDelete }: Props) => {
	const updateTag = useLibraryMutation('tags.update');
	const form = useZodForm({
		schema,
		mode: 'onChange',
		defaultValues: tag,
		reValidateMode: 'onChange'
	});
	useDebouncedFormWatch(form, (data) => {
		updateTag.mutate({
			name: data.name ?? null,
			color: data.color ?? null,
			id: tag.id
		});
	});

	return (
		<Form form={form}>
			<div className="flex justify-between">
				<div className="mb-10 flex flex-row space-x-3">
					<Input
						label="Color"
						maxLength={7}
						value={form.watch('color')?.trim() ?? '#ffffff'}
						icon={<ColorPicker control={form.control} name="color" />}
						{...form.register('color')}
					/>

					<Input maxLength={24} label="Name" {...form.register('name')} />
				</div>
				<Button
					variant="gray"
					className="mt-[22px] h-[38px]"
					onClick={() =>
						dialogManager.create((dp) => (
							<DeleteDialog {...dp} tagId={tag.id} onSuccess={onDelete} />
						))
					}
				>
					<Tooltip label="Delete Tag">
						<Trash className="h-4 w-4" />
					</Tooltip>
				</Button>
			</div>
			<Setting mini title="Show in Spaces" description="Show this tag on the spaces screen.">
				<Switch checked />
			</Setting>
		</Form>
	);
};
