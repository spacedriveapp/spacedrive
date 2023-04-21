import { Clipboard } from 'phosphor-react';
import { Button, Dialog, Input, UseDialogProps, dialogManager, useDialog } from '@sd/ui';
import { useZodForm, z } from '@sd/ui/src/forms';

interface Props extends UseDialogProps {
	title: string; // dialog title
	description?: string; // description of the dialog
	value: string; // value to be displayed as text or in an input box
	label?: string; // button label
	inputBox?: boolean; // whether the dialog should display the `value` in a disabled input box or as text
}

const AlertDialog = (props: Props) => {
	const dialog = useDialog(props);
	const form = useZodForm({ schema: z.object({}) });
	// maybe a copy-to-clipboard button would be beneficial too
	return (
		<Dialog
			title={props.title}
			form={form}
			onSubmit={form.handleSubmit(() => {})}
			dialog={dialog}
			description={props.description}
			ctaLabel={props.label !== undefined ? props.label : 'Done'}
		>
			{props.inputBox && (
				<Input
					value={props.value}
					disabled
					className="mt-3"
					right={
						<Button
							type="button"
							onClick={() => {
								navigator.clipboard.writeText(props.value);
							}}
							size="icon"
						>
							<Clipboard className="h-4 w-4" />
						</Button>
					}
				/>
			)}

			{!props.inputBox && <div className="text-sm">{props.value}</div>}
		</Dialog>
	);
};

export function showAlertDialog(props: Omit<Props, 'id'>) {
	dialogManager.create((dp) => <AlertDialog {...dp} {...props} />);
}
