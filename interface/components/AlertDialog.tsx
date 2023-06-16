import { Clipboard } from 'phosphor-react';
import { Button, Dialog, Input, UseDialogProps, dialogManager, useDialog } from '@sd/ui';
import { useZodForm } from '@sd/ui/src/forms';
import { ReactNode } from 'react';

interface Props extends UseDialogProps {
	title: string; // dialog title
	description?: string; // description of the dialog
	children?: ReactNode; // dialog content
	value?: string; // value to be displayed as text or in an input box
	label?: string; // button label
	inputBox?: boolean; // whether the dialog should display the `value` in a disabled input box or as text
}

const AlertDialog = (props: Props) => {
	// maybe a copy-to-clipboard button would be beneficial too
	return (
		<Dialog
			title={props.title}
			form={useZodForm()}
			dialog={useDialog(props)}
			ctaLabel={props.label !== undefined ? props.label : 'Done'}
			onCancelled={false}
		>
			{props.description && <div className="mb-3 text-sm">{props.description}</div>}
			{props.children}
			{props.inputBox ? (
				<Input
					value={props.value}
					disabled
					className="mt-3"
					right={
						<Button
							type="button"
							onClick={() => {
								props.value && navigator.clipboard.writeText(props.value);
							}}
							size="icon"
						>
							<Clipboard className="h-4 w-4" />
						</Button>
					}
				/>
			) : (
				<div className="text-sm">{props.value}</div>
			)}
		</Dialog>
	);
};

export function showAlertDialog(props: Omit<Props & { children?: ReactNode }, 'id'>) {
	dialogManager.create((dp) => <AlertDialog {...dp} {...props} />);
}
