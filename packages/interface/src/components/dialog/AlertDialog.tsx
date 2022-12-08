import { Dialog, Input } from '@sd/ui';

export interface AlertDialogProps {
	open: boolean;
	setOpen: (isShowing: boolean) => void;
	title: string; // dialog title
	description?: string; // description of the dialog
	value: string; // value to be displayed as text or in an input box
	label?: string; // button label
	inputBox: boolean; // whether the dialog should display the `value` in a disabled input box or as text
}

export const AlertDialog = (props: AlertDialogProps) => {
	// maybe a copy-to-clipboard button would be beneficial too
	return (
		<Dialog
			open={props.open}
			setOpen={props.setOpen}
			title={props.title}
			description={props.description}
			ctaAction={() => {
				props.setOpen(false);
			}}
			ctaLabel={props.label !== undefined ? props.label : 'Done'}
		>
			{props.inputBox === true && (
				<Input className="flex-grow w-full mt-3" value={props.value} disabled={true} />
			)}

			{props.inputBox === false && <div className="text-sm">{props.value}</div>}
		</Dialog>
	);
};
