import { Clipboard } from 'phosphor-react';
import { ReactNode } from 'react';
import { Button, Dialog, Input, UseDialogProps, dialogManager, useDialog } from '@sd/ui';
import { useZodForm } from '@sd/ui/src/forms';

interface Props extends UseDialogProps {
	title: string; // dialog title
	description?: string; // description of the dialog
	children?: ReactNode; // dialog content
	value?: string; // value to be displayed as text or in an input box
	label?: string; // button label
	inputBox?: boolean; // whether the dialog should display the `value` in a disabled input box or as text
	cancelBtn?: boolean; // whether the dialog should have a cancel button
}

const AlertDialog = (props: Props) => {
	// maybe a copy-to-clipboard button would be beneficial too
	return (
		<Dialog
			title={props.title}
			form={useZodForm()}
			dialog={useDialog(props)}
			ctaLabel={props.label !== undefined ? props.label : 'Done'}
			cancelBtn={props.cancelBtn}
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
							<Clipboard className="w-4 h-4" />
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
