import { dialogManager } from '@sd/ui';
import { AlertDialog, AlertDialogProps } from '~/components/dialog/AlertDialog';

export function showAlertDialog(props: AlertDialogProps) {
	dialogManager.create((dp) => <AlertDialog {...dp} {...props} />);
}
