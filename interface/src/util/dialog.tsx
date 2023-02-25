import { dialogManager } from '@sd/ui';
import { AlertDialog, AlertDialogProps } from '~/components/AlertDialog';

export function showAlertDialog(props: Omit<AlertDialogProps, 'id'>) {
	dialogManager.create((dp) => <AlertDialog {...dp} {...props} />);
}
