import { StoredKey, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Dialog, Input, Select, SelectOption } from '@sd/ui';
import { ReactNode, useMemo, useState } from 'react';

import { SelectOptionMountedKeys } from '../key/KeyList';

interface KeyViewerDialogProps {
	trigger: ReactNode;
}

export const KeyTextBox = (props: { uuid: string }) => {
	const keyValue = useMemo(() => useLibraryQuery(['keys.getKey', props.uuid]), [props.uuid]);

	return <Input className="flex-grow w-full mt-3" value={keyValue.data} disabled={true} />;
};

export const KeyViewerDialog = (props: KeyViewerDialogProps) => {
	const keys = useLibraryQuery(['keys.list']);
	const mountedUuids = useLibraryQuery(['keys.listMounted'], {
		onSuccess: (data) => {
			if (key === '' && data.length !== 0) {
				setKey(data[0]);
			}
		}
	});

	const [showKeyViewerDialog, setShowKeyViewerDialog] = useState(false);
	const [key, setKey] = useState('');

	// const UpdateKey = (uuid: string) => {
	// 	setKey(uuid);
	// 	const value = useLibraryQuery(['keys.getKey', uuid]);
	// 	value.data && setKeyValue(value.data);
	// };

	// const value = useLibraryQuery(['keys.getKey', key]);
	// value.data && setKeyValue(value.data);

	return (
		<>
			<Dialog
				open={showKeyViewerDialog}
				setOpen={setShowKeyViewerDialog}
				trigger={props.trigger}
				title="View Key Values"
				description="Here you can view the values of your keys."
				ctaLabel="Done"
				ctaAction={() => {
					// need to null things in here
					setShowKeyViewerDialog(false);
				}}
			>
				<div className="grid w-full gap-4 mt-4 mb-3">
					<div className="flex flex-col">
						<span className="text-xs font-bold">Key</span>
						<Select
							className="mt-2 flex-grow"
							value={key}
							onChange={(e) => {
								// UpdateKey(e);
								setKey(e);
							}}
						>
							{/* this only returns MOUNTED keys. we could include unmounted keys, but then we'd have to prompt the user to mount them too */}
							{keys.data && mountedUuids.data && (
								<SelectOptionMountedKeys keys={keys.data} mountedUuids={mountedUuids.data} />
							)}
						</Select>
					</div>
				</div>
				<div className="grid w-full gap-4 mt-4 mb-3">
					<div className="flex flex-col">
						<span className="text-xs font-bold">Value</span>
						<KeyTextBox uuid={key} />
					</div>
				</div>
			</Dialog>
		</>
	);
};
