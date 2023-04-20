import { ErrorMessage } from '@hookform/error-message';
import clsx from 'clsx';
import { CaretRight, Info, Plus, Trash } from 'phosphor-react';
import { ComponentProps, createRef, forwardRef, useEffect, useId, useState } from 'react';
import { createPortal } from 'react-dom';
import { Controller, ControllerRenderProps } from 'react-hook-form';
import {
	RuleKind,
	UnionToTuple,
	extractInfoRSPCError,
	isKeyOf,
	useLibraryMutation,
	useLibraryQuery
} from '@sd/client';
import {
	Button,
	Card,
	Divider,
	Input,
	Label,
	RadixCheckbox,
	Switch,
	Tabs,
	Tooltip,
	forms
} from '@sd/ui';
import { showAlertDialog } from '~/components/AlertDialog';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';
import { usePlatform } from '~/util/Platform';
import { openDirectoryPickerDialog } from './AddLocationDialog';

const { z, Form, useZodForm } = forms;

// NOTE: This should be updated whenever RuleKind is changed
const ruleKinds: UnionToTuple<RuleKind> = [
	'AcceptFilesByGlob',
	'RejectFilesByGlob',
	'AcceptIfChildrenDirectoriesArePresent',
	'RejectIfChildrenDirectoriesArePresent'
];

interface RulesInputProps {
	form: string;
	onChange: ComponentProps<'input'>['onChange'];
	className: string;
	onInvalid: ComponentProps<'input'>['onInvalid'];
}

const RuleTabsInput = {
	Name: forwardRef<HTMLInputElement, RulesInputProps>((props, ref) => {
		const os = useOperatingSystem(true);
		return (
			<Input
				ref={ref}
				size="md"
				// TODO: The check here shouldn't be for which os the UI is running, but for which os the node is running
				pattern={os === 'windows' ? '[^<>:"/\\|?*\u0000-\u0031]*' : '[^/\0]+'}
				placeholder="File/Directory name"
				{...props}
			/>
		);
	}),
	Extension: forwardRef<HTMLInputElement, RulesInputProps>((props, ref) => (
		<Input
			ref={ref}
			size="md"
			pattern="^\.[^\.\s]+$"
			aria-label="Add a file extension to the current rule"
			placeholder="File extension (e.g., .mp4, .jpg, .txt)"
			{...props}
		/>
	)),
	Path: forwardRef<HTMLInputElement, RulesInputProps>(({ className, ...props }, ref) => {
		const os = useOperatingSystem(true);
		const platform = usePlatform();
		const isWeb = platform.platform === 'web';
		return (
			<Input
				ref={ref}
				size="md"
				pattern={
					isWeb
						? // Non web plataforms use the native file picker, so there is no need to validate
						  ''
						: // TODO: The check here shouldn't be for which os the UI is running, but for which os the node is running
						os === 'windows'
						? '[^<>:"/|?*\u0000-\u0031]*'
						: '[^\0]+'
				}
				readOnly={!isWeb}
				className={clsx(className, isWeb || 'cursor-pointer')}
				placeholder={
					'Path (e.g., ' +
					(os === 'windows'
						? 'C:\\Users\\john\\Downloads'
						: os === 'macOS'
						? '/Users/clara/Pictures'
						: '/home/emily/Documents') +
					')'
				}
				onClick={(e) => {
					openDirectoryPickerDialog(platform)
						.then((path) => {
							if (path) (e.target as HTMLInputElement).value = path;
						})
						.catch((error) =>
							showAlertDialog({
								title: 'Error',
								value: String(error)
							})
						);
				}}
				{...props}
			/>
		);
	}),
	Advanced: forwardRef<HTMLInputElement, RulesInputProps>((props, ref) => {
		const os = useOperatingSystem(true);
		return (
			<Input
				ref={ref}
				size="md"
				pattern={
					// TODO: The check here shouldn't be for which os the UI is running, but for which os the node is running
					os === 'windows' ? '[^<>:"\u0000-\u0031]*' : '[^\0]+'
				}
				placeholder="Glob (e.g., **/.git)"
				{...props}
			/>
		);
	})
};

type RuleType = keyof typeof RuleTabsInput;

type ParametersFieldType = ControllerRenderProps<
	{ parameters: [RuleType, string][] },
	'parameters'
>;

interface RuleTabsContentProps<T extends ParametersFieldType> {
	form: string;
	field: T;
	value: RuleType;
}

function RuleTabsContent<T extends ParametersFieldType>({
	form,
	value,
	field,
	...props
}: RuleTabsContentProps<T>) {
	const [invalid, setInvalid] = useState(false);
	const inputRef = createRef<HTMLInputElement>();
	const RuleInput = RuleTabsInput[value];

	return (
		<Tabs.Content asChild value={value} {...props}>
			<div className="flex flex-row justify-between pt-4">
				<RuleInput
					ref={inputRef}
					form={form}
					onChange={(e) => {
						const input = e.target;
						setInvalid(false);

						// Even if the input value is valid, without clearing the custom validity, the invalid state will remain
						input.setCustomValidity('');

						input.reportValidity();
					}}
					onInvalid={(e) => {
						// Required to prevent the browser from showing the default error message
						(e.target as HTMLInputElement).setCustomValidity(' ');
						setInvalid(true);
					}}
					className={clsx('mr-2 flex-1', invalid && '!ring-2 !ring-red-500')}
				/>
				<Button
					onClick={() => {
						const { current: input } = inputRef;
						if (!(input && input.checkValidity()) || input.value.trim() === '') return;
						field.onChange([...field.value, [value, input.value]]);
						input.value = '';
					}}
					variant="accent"
				>
					<Plus />
				</Button>
			</div>
		</Tabs.Content>
	);
}

type IndexerRuleIdFieldType = ControllerRenderProps<
	{ indexerRulesIds: number[] },
	'indexerRulesIds'
>;

export interface IndexerRuleEditorProps<T extends IndexerRuleIdFieldType> {
	field?: T;
	editable?: boolean;
}

const ruleKindEnum = z.enum(ruleKinds);

const newRuleSchema = z.object({
	kind: ruleKindEnum,
	name: z.string(),
	parameters: z.array(
		z.tuple([z.enum(Object.keys(RuleTabsInput) as UnionToTuple<RuleType>), z.string()])
	)
});

const REMOTE_ERROR_FORM_FIELD = 'root.serverError';

const removeParameter = <T extends ParametersFieldType>(field: T, index: number) =>
	field.onChange(field.value.slice(0, index).concat(field.value.slice(index + 1)));

export function IndexerRuleEditor<T extends IndexerRuleIdFieldType>({
	field,
	editable
}: IndexerRuleEditorProps<T>) {
	const os = useOperatingSystem(true);
	const form = useZodForm({
		schema: newRuleSchema,
		defaultValues: { kind: 'RejectFilesByGlob', name: '', parameters: [] }
	});
	const formId = useId();
	const listIndexerRules = useLibraryQuery(['locations.indexer_rules.list']);
	const createIndexerRules = useLibraryMutation(['locations.indexer_rules.create']);
	const [currentTab, setCurrentTab] = useState<RuleType>('Name');
	const [showCreateNewRule, setShowCreateNewRule] = useState(false);

	useEffect(() => {
		// TODO: Instead of clearing the error on every change, the backend should suport a way to validate without committing
		const subscription = form.watch(() => {
			form.clearErrors(REMOTE_ERROR_FORM_FIELD);
		});
		return () => subscription.unsubscribe();
	}, [form]);

	const onSubmit = form.handleSubmit(async ({ kind, name, parameters }) =>
		createIndexerRules.mutateAsync({
			kind,
			name,
			parameters: parameters.map(([kind, rule]) => {
				switch (kind) {
					case 'Name':
						rule = `**/${rule}`;
						break;
					case 'Extension':
						rule = `**/*.${rule}`;
						break;
				}

				return rule;
			})
		})
	);

	const onSubmitError = (error: Error) => {
		const rspcErrorInfo = extractInfoRSPCError(error);
		if (rspcErrorInfo && rspcErrorInfo.code !== 500) {
			form.reset({}, { keepValues: true, keepErrors: true, keepIsValid: true });
			form.setError(REMOTE_ERROR_FORM_FIELD, {
				type: 'remote',
				message: rspcErrorInfo.message
			});
		} else {
			showAlertDialog({
				title: 'Error',
				value: String(error) || 'Failed to add location'
			});
		}
	};

	const indexRules = listIndexerRules.data;
	return (
		<>
			<Card className="mb-2 flex flex-wrap justify-evenly">
				{indexRules ? (
					indexRules.map((rule) => {
						const { id, name } = rule;
						const enabled = field?.value.includes(id) ?? false;
						return (
							<Button
								key={id}
								size="sm"
								onClick={
									field &&
									(() =>
										field.onChange(
											enabled
												? field.value.filter(
														(fieldValue) => fieldValue !== rule.id
												  )
												: Array.from(new Set([...field.value, rule.id]))
										))
								}
								variant={enabled ? 'colored' : 'outline'}
								disabled={!field}
								className={clsx(
									'm-1 flex-auto',
									enabled && 'border-accent bg-accent'
								)}
							>
								{name}
							</Button>
						);
					})
				) : (
					<p className={clsx(listIndexerRules.isError && 'text-red-500')}>
						{listIndexerRules.isError
							? 'Error while retriving indexer rules'
							: 'No indexer rules available'}
					</p>
				)}
			</Card>
			{
				// Portal is required because this component can be inside another form element
				createPortal(
					<Form
						id={formId}
						form={form}
						onSubmit={(e) =>
							onSubmit(e).then(() => {
								listIndexerRules.refetch();
							}, onSubmitError)
						}
						className="hidden h-0 w-0"
					/>,
					document.body
				)
			}
			{editable && (
				<div className="rounded-md border border-app-line bg-app-overlay">
					<Button
						variant="bare"
						className={clsx(
							'flex w-full border-none !p-3',
							showCreateNewRule && 'rounded-b-none'
						)}
						onClick={() => setShowCreateNewRule(!showCreateNewRule)}
					>
						Create new indexer rule
						<CaretRight
							weight="bold"
							className={clsx('ml-1 transition', showCreateNewRule && 'rotate-90')}
						/>
					</Button>

					{showCreateNewRule && (
						<div className="px-4 pb-4 pt-2">
							<h3 className="w-full text-center text-sm font-semibold">Rules</h3>
							<Divider className="mb-2" />
							<Controller
								name="parameters"
								rules={{ required: true }}
								render={({ field }) => (
									<>
										<div className="grid space-y-1 rounded-md border border-app-line/60 bg-app-overlay p-2">
											{((rules) =>
												rules.length === 0 ? (
													<p className="w-full p-2 text-center text-sm text-ink-dull">
														No rules yet
													</p>
												) : (
													rules.map(([kind, rule], index) => (
														<Card
															key={index}
															className="border-app-line/30 hover:bg-app-box/70"
														>
															<p className="p-1 text-sm font-semibold text-ink-dull">
																{rule}
															</p>
															<div className="grow" />
															<p className="mx-2 rounded-md border border-app-line/30 bg-app-overlay/80 py-1 px-2 text-center text-sm text-ink-dull">
																{kind}
															</p>
															<Button
																variant="gray"
																onClick={() =>
																	removeParameter(field, index)
																}
															>
																<Tooltip label="Delete rule">
																	<Trash size={14} />
																</Tooltip>
															</Button>
														</Card>
													))
												))(form.getValues().parameters)}
										</div>
										<Tabs.Root
											value={currentTab}
											onValueChange={(tab) =>
												isKeyOf(tab, RuleTabsInput) && setCurrentTab(tab)
											}
										>
											<Tabs.List className="flex flex-row">
												{Object.keys(RuleTabsInput).map((name) => (
													<Tabs.Trigger
														className="flex-auto py-2 text-sm font-medium"
														key={name}
														value={name}
													>
														{name}
													</Tabs.Trigger>
												))}
											</Tabs.List>
											{...(Object.keys(RuleTabsInput) as RuleType[]).map(
												(name) => (
													<RuleTabsContent
														key={name}
														form={formId}
														value={name}
														field={field}
													/>
												)
											)}
										</Tabs.Root>
									</>
								)}
								control={form.control}
							/>
							<h3 className="mt-4 w-full text-center text-sm font-semibold">
								Settings
							</h3>
							<Divider className="mb-2" />
							<div className="mb-2 flex flex-row justify-between">
								<div className="mr-2 grow">
									<Input
										size="md"
										form={formId}
										required
										className="mb-2"
										placeholder="Name"
										{...form.register('name')}
									/>
									<div className="flex w-full flex-row">
										<label className="grow text-sm font-medium">
											Indexer rule is an allow list{' '}
											<Tooltip label="By default, an indexer rule acts as a deny list, causing a location to ignore any file that match its rules. Enabling this will make it act as an allow list, and the location will only display files that match its rules.">
												<Info className="inline" />
											</Tooltip>
										</label>
										<Controller
											name="kind"
											render={({ field }) => (
												<Switch
													onCheckedChange={(checked) => {
														let { value } = field;
														const kind = ruleKindEnum.enum;
														switch (value) {
															case kind.RejectFilesByGlob:
															case kind.AcceptFilesByGlob:
																value = checked
																	? kind.AcceptFilesByGlob
																	: kind.RejectFilesByGlob;
																break;
															case kind.AcceptIfChildrenDirectoriesArePresent:
															case kind.RejectIfChildrenDirectoriesArePresent:
																value = checked
																	? kind.AcceptIfChildrenDirectoriesArePresent
																	: kind.RejectIfChildrenDirectoriesArePresent;
																break;
														}
														field.onChange(value);
													}}
													size="sm"
												/>
											)}
											control={form.control}
										/>
									</div>
								</div>
								<Button
									size="sm"
									type="submit"
									variant="accent"
									disabled={form.formState.isSubmitting}
									className="mb-7"
								>
									<Plus />
								</Button>
							</div>
							<ErrorMessage
								name={REMOTE_ERROR_FORM_FIELD}
								render={({ message }) => (
									<span className="mt-5 inline-block w-full whitespace-pre-wrap text-center text-sm font-semibold text-red-500">
										{message}
									</span>
								)}
							/>
						</div>
					)}
				</div>
			)}
		</>
	);
}
