import { Dialog } from '@sd/ui';

export const ExplorerAlertDialog = (props: {
	open: boolean;
	setOpen: (isShowing: boolean) => void;
	title: string;
	description?: string;
	text: string;
}) => {
	const { open, setOpen, title, description, text } = props;
	return (
		<>
			<Dialog
				open={open}
				setOpen={setOpen}
				title={title}
				description={description}
				ctaLabel="Done"
				ctaAction={() => {
					setOpen(false);
				}}
			>
				<div className="text-sm">{text}</div>
			</Dialog>
		</>
	);
};
