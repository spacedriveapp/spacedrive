import { useLibraryQuery } from '@sd/client';
import { Dialog, Input, Select } from '@sd/ui';
import { ReactNode, useEffect, useMemo, useState } from 'react';

import { SelectOptionKeys } from '../key/KeyList';

interface KeyViewerDialogProps {
	trigger: ReactNode;
}

export const KeyTextBox = (props: { uuid: string }) => {
	const kV = useLibraryQuery(['keys.getKey', props.uuid]);

	const [keyValue, setKeyValue] = useState('');

	useEffect(() => {
		kV.data && setKeyValue(kV.data);
	}, [kV]);

	return <Input className="flex-grow w-full mt-3" value={keyValue} disabled={true} />;
};

export const KeyViewerDialog = (props: KeyViewerDialogProps) => {
	const keys = useLibraryQuery(['keys.list'], {
		onSuccess: (data) => {
			if (key === '' && data.length !== 0) {
				setKey(data[0].uuid);
			}
		}
	});

	const [showKeyViewerDialog, setShowKeyViewerDialog] = useState(false);
	const [key, setKey] = useState('');

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
								setKey(e);
							}}
						>
							{keys.data && <SelectOptionKeys keys={keys.data} />}
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
