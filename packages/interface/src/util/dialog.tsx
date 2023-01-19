import { dialogManager } from '@sd/ui';
import { AlertDialog, GenericAlertDialogProps } from '~/components/dialog/AlertDialog';

export function showAlertDialog(props: GenericAlertDialogProps) {
	dialogManager.create((dp) => <AlertDialog {...dp} {...props} />);
}
