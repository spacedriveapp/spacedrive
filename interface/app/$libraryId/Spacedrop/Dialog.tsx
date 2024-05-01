import { useZodForm } from '@sd/client';
import { Dialog, Divider, useDialog, UseDialogProps, z } from '@sd/ui';
import { Icon } from '~/components';
import { useLocale } from '~/hooks';

export default function SpacedropDialog(props: UseDialogProps) {
	const { t } = useLocale();

	const form = useZodForm({ schema: z.object({}) });

	return (
		<Dialog form={form} dialog={useDialog(props)} hideButtons>
			<div className="flex w-full flex-col items-center space-y-2 p-4">
				<div className="flex flex-row items-center gap-2">
					<Icon name="Spacedrop" size={36} />
					<span className="text-lg font-bold">Spacedrop</span>
				</div>
				<p className="text-balance text-center text-sm text-ink-dull">
					{t('spacedrop_description')}
				</p>
			</div>
			<Divider />
		</Dialog>
	);
}
