import {
	ExplorerItem,
	FilePath,
	libraryClient,
	Object,
	Target,
	useLibraryMutation,
	usePlausibleEvent,
	useZodForm
} from '@sd/client';
import { Dialog, InputField, useDialog, UseDialogProps, z } from '@sd/ui';
import { ColorPicker } from '~/components';

const schema = z.object({
	name: z.string().trim().min(1).max(24),
	color: z.string()
});

export type AssignTagItems = Array<
	{ type: 'Object'; item: Object } | { type: 'Path'; item: FilePath }
>;

export async function assignItemsToTag(
	client: typeof libraryClient,
	tagId: number,
	items: AssignTagItems,
	unassign: boolean
) {
	const targets = items.map<Target>((item) => {
		if (item.type === 'Object') {
			return { Object: item.item.id };
		} else {
			return { FilePath: item.item.id };
		}
	});

	await client.mutation([
		'tags.assign',
		{
			targets,
			tag_id: tagId,
			unassign
		}
	]);
}

export default (
	props: UseDialogProps & {
		items?: AssignTagItems;
	}
) => {
	const submitPlausibleEvent = usePlausibleEvent();

	const form = useZodForm({
		schema: schema,
		defaultValues: { color: '#A717D9' },
		mode: 'onBlur'
	});

	const createTag = useLibraryMutation('tags.create');

	const onSubmit = form.handleSubmit(async (data) => {
		try {
			const tag = await createTag.mutateAsync(data);

			submitPlausibleEvent({ event: { type: 'tagCreate' } });

			if (props.items !== undefined)
				await assignItemsToTag(libraryClient, tag.id, props.items, false);
		} catch (e) {
			console.error('error', e);
		}
	});

	return (
		<Dialog
			invertButtonFocus
			form={form}
			onSubmit={onSubmit}
			dialog={useDialog(props)}
			title="Create New Tag"
			description="Choose a name and color."
			ctaLabel="Create"
		>
			<div className="relative mt-3 ">
				<InputField
					{...form.register('name', { required: true })}
					placeholder="Name"
					maxLength={24}
					icon={<ColorPicker control={form.control} name="color" />}
				/>
			</div>
		</Dialog>
	);
};
