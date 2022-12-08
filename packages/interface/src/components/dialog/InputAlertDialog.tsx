import { Dialog, Input } from '@sd/ui';

export interface GenericTextInputDialogProps {
	open: boolean;
	setOpen: (isShowing: boolean) => void;
	title: string;
	description?: string;
	value: string;
	label?: string;
}

export const InputAlertDialog = (props: GenericTextInputDialogProps) => {
	// maybe a copy-to-clipboard button would be beneficial too
	return (
		<Dialog
			open={props.open}
			setOpen={props.setOpen}
			title="Secret Key"
			description={props.description}
			ctaAction={() => {
				props.setOpen(false);
			}}
			ctaLabel={props.label !== undefined ? props.label : 'Done'}
		>
			<Input className="flex-grow w-full mt-3" value={props.value} disabled={true} />
		</Dialog>
	);
};
