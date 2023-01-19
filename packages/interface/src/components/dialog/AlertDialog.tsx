import { Button, Dialog, Input, NewDialogProps, useDialog } from '@sd/ui';
import { Clipboard } from 'phosphor-react';

import { useZodForm, z } from '@sd/ui/src/forms';

export const GenericAlertDialogState = {
	title: '',
	description: '',
	value: '',
	inputBox: false
};

export interface GenericAlertDialogProps {
	title: string;
	description?: string;
	value: string;
	inputBox?: boolean;
}

export interface AlertDialogProps extends NewDialogProps {
	title: string; // dialog title
	description?: string; // description of the dialog
	value: string; // value to be displayed as text or in an input box
	label?: string; // button label
	inputBox?: boolean; // whether the dialog should display the `value` in a disabled input box or as text
}

export const AlertDialog = (props: AlertDialogProps) => {
	const dialog = useDialog(props);
	const form = useZodForm({ schema: z.object({}) });
	// maybe a copy-to-clipboard button would be beneficial too
	return (
		<Dialog
			form={form}
			onSubmit={form.handleSubmit(() => {})}
			dialog={dialog}
			description={props.description}
			ctaLabel={props.label !== undefined ? props.label : 'Done'}
		>
			{props.inputBox && (
				<div className="relative flex flex-grow mt-3">
					<Input value={props.value} disabled className="flex-grow !py-0.5" />
					<Button
						type="button"
						onClick={() => {
							navigator.clipboard.writeText(props.value);
						}}
						size="icon"
						className="border-none absolute right-[5px] top-[5px]"
					>
						<Clipboard className="w-4 h-4" />
					</Button>
				</div>
			)}

			{!props.inputBox && <div className="text-sm">{props.value}</div>}
		</Dialog>
	);
};
