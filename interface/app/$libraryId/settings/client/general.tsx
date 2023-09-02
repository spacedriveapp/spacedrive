import { Laptop } from '@sd/assets/icons';
import {
	getDebugState,
	useBridgeMutation,
	useBridgeQuery,
	useDebugState,
	useZodForm
} from '@sd/client';
import { Button, Card, Input, Switch, tw, z } from '@sd/ui';
import { useDebouncedFormWatch } from '~/hooks';
import { usePlatform } from '~/util/Platform';
import { Heading } from '../Layout';
import Setting from '../Setting';

const NodePill = tw.div`px-1.5 py-[2px] rounded text-xs font-medium bg-app-selected`;
const NodeSettingLabel = tw.div`mb-1 text-xs font-medium`;

export const Component = () => {
	const node = useBridgeQuery(['nodeState']);
	const platform = usePlatform();
	const debugState = useDebugState();
	const editNode = useBridgeMutation('nodes.edit');

	const form = useZodForm({
		schema: z.object({
			name: z.string().min(1)
		}),
		defaultValues: {
			name: node.data?.name || ''
		}
	});

	useDebouncedFormWatch(form, async (value) => {
		await editNode.mutateAsync({
			name: value.name || null
		});

		node.refetch();
	});

	return (
		<>
			<Heading
				title="General Settings"
				description="General settings related to this client."
			/>
			<Card className="px-5">
				<div className="my-2 flex w-full flex-col">
					<div className="flex flex-row items-center justify-between">
						<span className="font-semibold">Connected Node</span>
						<div className="flex flex-row space-x-1">
							<NodePill>0 Peers</NodePill>
							<NodePill className="!bg-accent text-white">Running</NodePill>
						</div>
					</div>

					<hr className="mb-4 mt-2 flex  w-full border-app-line" />
					<div className="flex w-full items-center gap-5">
						<img src={Laptop} className="mt-2 h-14 w-14" />

						<div className="flex flex-col">
							<NodeSettingLabel>Node Name</NodeSettingLabel>
							<Input
								{...form.register('name', { required: true })}
								defaultValue={node.data?.name}
							/>
						</div>
						{/* <div className="flex flex-col">
							<NodeSettingLabel>Node Port</NodeSettingLabel>
							<Input
								contentEditable={false}
								value={node.data?.p2p_port || 5795}
								onChange={() => {
									alert('TODO');
								}}
							/>
						</div> */}
					</div>

					<div className="mt-6 gap-2">
						{/* <div
							onClick={() => {
								if (node.data && platform?.openLink) {
									platform.openLink(node.data.data_path);
								}
							}}
							className="text-sm font-medium text-ink-faint"
						>
							<b className="mr-2 inline truncate">
								<Database className="mr-1 mt-[-2px] inline h-4 w-4" /> Data Folder
							</b>
							<span className="select-text">{node.data?.data_path}</span>
						</div> */}

						<div>
							<NodeSettingLabel>Data Folder</NodeSettingLabel>
							<div className="mt-2 flex w-full flex-row gap-2">
								<Input className="grow" value={node.data?.data_path} disabled />
								<Button
									size="sm"
									variant="outline"
									onClick={() => {
										if (node.data && !!platform?.openLink) {
											platform.confirm(
												'Modifying or backing up data within this folder may cause irreparable damage! Proceed at your own risk!',
												(result) => {
													if (result) {
														platform.openLink(node.data.data_path);
													}
												}
											);
										}
									}}
								>
									Open
								</Button>
								{/* <Button size="sm" variant="outline">
									Change
								</Button> */}
							</div>
						</div>
						{/* <div className='mb-1'>
							<Label className="text-sm font-medium text-ink-faint">
								<Database className="mr-1 mt-[-2px] inline h-4 w-4" /> Logs Folder
							</Label>
							<Input value={node.data?.data_path + '/logs'} />
						</div> */}
					</div>
					{/* <div className="pointer-events-none mt-5 flex items-center space-x-3 opacity-50">
						<Switch size="sm" />
						<span className="text-sm font-medium text-ink-dull">
							Run Spacedrive in the background when app closed
						</span>
					</div> */}
				</div>
			</Card>
			{(isDev || debugState.enabled) && (
				<Setting
					mini
					title="Debug mode"
					description="Enable extra debugging features within the app."
				>
					<Switch
						size="md"
						checked={debugState.enabled}
						onClick={() => (getDebugState().enabled = !debugState.enabled)}
					/>
				</Setting>
			)}
		</>
	);
};
