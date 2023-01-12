import { ExplorerData, rspc, useCurrentLibrary } from '@sd/client';
import { useEffect, useState } from 'react';

import { useExplorerStore } from '../../hooks/useExplorerStore';
import { AlertDialog, GenericAlertDialogState } from '../dialog/AlertDialog';
import { DecryptFileDialog } from '../dialog/DecryptFileDialog';
import { DeleteFileDialog } from '../dialog/DeleteFileDialog';
import { EncryptFileDialog } from '../dialog/EncryptFileDialog';
import { EraseFileDialog } from '../dialog/EraseFileDialog';
import { Inspector } from '../explorer/Inspector';
import { ExplorerContextMenu } from './ExplorerContextMenu';
import { TopBar } from './ExplorerTopBar';
import { VirtualizedList } from './VirtualizedList';

interface Props {
	data?: ExplorerData;
}

export default function Explorer(props: Props) {
	const expStore = useExplorerStore();
	const { library } = useCurrentLibrary();

	const [scrollSegments, setScrollSegments] = useState<{ [key: string]: number }>({});
	const [separateTopBar, setSeparateTopBar] = useState<boolean>(false);

	const [showEncryptDialog, setShowEncryptDialog] = useState(false);
	const [showDecryptDialog, setShowDecryptDialog] = useState(false);
	const [showDeleteDialog, setShowDeleteDialog] = useState(false);
	const [showEraseDialog, setShowEraseDialog] = useState(false);

	const [alertDialogData, setAlertDialogData] = useState(GenericAlertDialogState);

	useEffect(() => {
		setSeparateTopBar((oldValue) => {
			const newValue = Object.values(scrollSegments).some((val) => val >= 5);

			if (newValue !== oldValue) return newValue;
			return oldValue;
		});
	}, [scrollSegments]);

	rspc.useSubscription(['jobs.newThumbnail', { library_id: library!.uuid, arg: null }], {
		onData: (cas_id) => {
			expStore.addNewThumbnail(cas_id);
		}
	});

	const selectedItem = props.data?.items[expStore.selectedRowIndex];

	return (
		<>
			<div className="relative">
				<ExplorerContextMenu>
					<div className="relative flex flex-col w-full">
						<TopBar showSeparator={separateTopBar} />

						<div className="relative flex flex-row w-full max-h-full app-background">
							{props.data && (
								<VirtualizedList
									data={props.data.items || []}
									context={props.data.context}
									onScroll={(y) => {
										setScrollSegments((old) => {
											return {
												...old,
												mainList: y
											};
										});
									}}
									setShowEncryptDialog={setShowEncryptDialog}
									setShowDecryptDialog={setShowDecryptDialog}
									setShowDeleteDialog={setShowDeleteDialog}
									setShowEraseDialog={setShowEraseDialog}
									setAlertDialogData={setAlertDialogData}
								/>
							)}
							{expStore.showInspector && (
								<div className="flex min-w-[260px] max-w-[260px]">
									<Inspector
										onScroll={(e) => {
											const y = (e.target as HTMLElement).scrollTop;

											setScrollSegments((old) => {
												return {
													...old,
													inspector: y
												};
											});
										}}
										key={props.data?.items[expStore.selectedRowIndex]?.id}
										data={props.data?.items[expStore.selectedRowIndex]}
									/>
								</div>
							)}
						</div>
					</div>
				</ExplorerContextMenu>
			</div>
			<AlertDialog
				open={alertDialogData.open}
				setOpen={(e) => {
					setAlertDialogData({ ...alertDialogData, open: e });
				}}
				title={alertDialogData.title}
				value={alertDialogData.value}
				inputBox={alertDialogData.inputBox}
			/>

			{/* these props are all shared so could use the same prop type */}
			{selectedItem && (
				<EncryptFileDialog
					location_id={expStore.locationId}
					path_id={selectedItem.id}
					open={showEncryptDialog}
					setOpen={setShowEncryptDialog}
					setAlertDialogData={setAlertDialogData}
				/>
			)}
			{selectedItem && expStore.locationId !== null && (
				<DecryptFileDialog
					location_id={expStore.locationId}
					path_id={selectedItem.id}
					open={showDecryptDialog}
					setOpen={setShowDecryptDialog}
					setAlertDialogData={setAlertDialogData}
				/>
			)}
			{selectedItem && expStore.locationId !== null && (
				<DeleteFileDialog
					location_id={expStore.locationId}
					path_id={selectedItem.id}
					open={showDeleteDialog}
					setOpen={setShowDeleteDialog}
				/>
			)}
			{selectedItem && expStore.locationId !== null && (
				<EraseFileDialog
					location_id={expStore.locationId}
					path_id={selectedItem.id}
					open={showEraseDialog}
					setOpen={setShowEraseDialog}
				/>
			)}
		</>
	);
}
