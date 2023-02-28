import { Trash } from 'phosphor-react';
import { Tag, useLibraryMutation } from '@sd/client';
import { Button, Switch, Tooltip, dialogManager } from '@sd/ui';
import { Form, Input, useZodForm, z } from '@sd/ui/src/forms';
import ColorPicker from '~/components/ColorPicker';
import { useDebouncedFormWatch } from '~/hooks/useDebouncedForm';
import Setting from '../../Setting';
import DeleteDialog from './DeleteDialog';

const schema = z.object({
	name: z.string().nullable(),
	color: z.string().nullable()
});

interface Props {
	tag: Tag;
	onDelete: () => void;
}

export default ({ tag, onDelete }: Props) => {
	const updateTag = useLibraryMutation('tags.update');

	const form = useZodForm({
		schema,
		defaultValues: tag
	});

	useDebouncedFormWatch(form, (data) =>
		updateTag.mutate({
			name: data.name ?? null,
			color: data.color ?? null,
			id: tag.id
		})
	);

	return (
		<Form form={form}>
			<div className="mb-10 flex flex-row space-x-3">
				<div className="flex flex-col">
					<span className="mb-1 text-sm font-medium text-gray-700 dark:text-gray-100">Color</span>
					<div className="relative">
						<ColorPicker className="!absolute left-[9px] top-[-5px]" {...form.register('color')} />
						<Input className="w-28 pl-[40px]" {...form.register('color')} />
					</div>
				</div>
				<div className="flex flex-col">
					<span className="mb-1 text-sm font-medium text-gray-700 dark:text-gray-100">Name</span>
					<Input {...form.register('name')} />
				</div>
				<div className="flex grow" />
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
