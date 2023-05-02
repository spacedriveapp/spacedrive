import clsx from 'clsx';
import { CaretRight, Info, Plus, Trash, X } from 'phosphor-react';
import { ComponentProps, createRef, forwardRef, useCallback, useId, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Controller, ControllerRenderProps, FormProvider } from 'react-hook-form';
import {
	IndexerRule,
	RuleKind,
	UnionToTuple,
	extractInfoRSPCError,
	isKeyOf,
	useLibraryMutation,
	useLibraryQuery
} from '@sd/client';
import { Button, Card, Divider, Input, Switch, Tabs, Tooltip, inputSizes } from '@sd/ui';
import { ErrorMessage, Form, Input as FormInput, useZodForm, z } from '@sd/ui/src/forms';
import { InfoPill } from '~/app/$libraryId/Explorer/Inspector';
import { showAlertDialog } from '~/components';
import { useCallbackToWatchForm, useOperatingSystem } from '~/hooks';
import { usePlatform } from '~/util/Platform';
import { openDirectoryPickerDialog } from './AddLocationDialog';

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

type IndexerRuleIdFieldType = ControllerRenderProps<
	{ indexerRulesIds: number[] },
	'indexerRulesIds'
>;

interface RuleButtonProps<T extends IndexerRuleIdFieldType> {
	rule: IndexerRule;
	field?: T;
	editable?: boolean;
	disabled?: boolean;
}

function RuleButton<T extends IndexerRuleIdFieldType>({
	rule,
	field,
	editable,
	disabled
}: RuleButtonProps<T>) {
	const timeoutId = useRef<number>(0);
	const [willDelete, setWillDelete] = useState<boolean>(false);
	const [isDeleting, setIsDeleting] = useState<boolean>(false);
	const listIndexerRules = useLibraryQuery(['locations.indexer_rules.list']);
	const deleteIndexerRule = useLibraryMutation(['locations.indexer_rules.delete']);

	const value = field?.value ?? [];
	const ruleEnabled = value.includes(rule.id);

	return (
		<Button
			size="sm"
			onClick={
				field &&
				(() =>
					field.onChange(
						ruleEnabled
							? value.filter((v) => v !== rule.id)
							: Array.from(new Set([...value, rule.id]))
					))
			}
			variant={disabled ? 'outline' : ruleEnabled ? 'accent' : 'colored'}
			disabled={disabled || isDeleting || !field}
			className={clsx('relative m-1 flex-auto overflow-hidden')}
		>
			{rule.name}
			{editable && !rule.default && (
				<div
					onClick={(e) => {
						e.stopPropagation();
						e.preventDefault();
						if (willDelete) {
							setIsDeleting(true);
							deleteIndexerRule
								.mutateAsync(rule.id)
								.then(
									() => listIndexerRules.refetch(),
									(error) =>
										showAlertDialog({
											title: 'Error',
											value: String(error) || 'Failed to add location'
										})
								)
								.finally(() => {
									setWillDelete(false);
									setIsDeleting(false);
								});
						} else {
							setWillDelete(true);
						}
					}}
					onMouseEnter={() => {
						const id = timeoutId.current;
						timeoutId.current = 0;
						if (id) clearTimeout(id);
					}}
					onMouseLeave={() => {
						timeoutId.current = setTimeout(() => {
							timeoutId.current = 0;
							if (!isDeleting) setWillDelete(false);
						}, 500);
					}}
					className={clsx(
						'absolute right-0 top-0 flex h-full cursor-pointer content-center items-center justify-center justify-items-center overflow-hidden bg-red-500 transition-[width]',
						willDelete ? 'w-full' : 'w-4'
					)}
				>
					{willDelete ? 'Delete?' : <X className="!pointer-events-none" />}
				</div>
			)}
		</Button>
	);
}

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
					// TODO: The check here shouldn't be for which os the UI is running, but for which os the node is running
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

export interface IndexerRuleEditorProps<T extends IndexerRuleIdFieldType> {
	field?: T;
	editable?: boolean;
}

const ruleKindEnum = z.enum(ruleKinds);

const schema = z.object({
	kind: ruleKindEnum,
	name: z.string().min(3),
	parameters: z
		.array(z.tuple([z.enum(Object.keys(RuleTabsInput) as UnionToTuple<RuleType>), z.string()]))
		.nonempty()
});

type SchemaType = z.infer<typeof schema>;

const REMOTE_ERROR_FORM_FIELD = 'root.serverError';

const removeParameter = <T extends ParametersFieldType>(field: T, index: number) =>
	field.onChange(field.value.slice(0, index).concat(field.value.slice(index + 1)));

export function IndexerRuleEditor<T extends IndexerRuleIdFieldType>({
	field,
	editable
}: IndexerRuleEditorProps<T>) {
	const form = useZodForm({
		schema: schema,
		defaultValues: { name: '', kind: 'RejectFilesByGlob', parameters: [] }
	});
	const formId = useId();
	const listIndexerRules = useLibraryQuery(['locations.indexer_rules.list']);
	const createIndexerRules = useLibraryMutation(['locations.indexer_rules.create']);
	const [currentTab, setCurrentTab] = useState<RuleType>('Name');
	const [showCreateNewRule, setShowCreateNewRule] = useState(false);

	const addIndexerRules = useCallback(
		({ kind, name, parameters }: SchemaType, dryRun = false) =>
			createIndexerRules.mutateAsync({
				kind,
				name,
				dry_run: dryRun,
				parameters: parameters.flatMap(([kind, rule]) => {
					switch (kind) {
						case 'Name':
							return `**/${rule}`;
						case 'Extension':
							// .tar should work for .tar.gz, .tar.bz2, etc.
							return [`**/*${rule}`, `**/*${rule}.*`];
						default:
							return rule;
					}
				})
			}),
		[createIndexerRules]
	);

	const handleAddError = useCallback(
		(error: unknown) => {
			const rspcErrorInfo = extractInfoRSPCError(error);
			if (!rspcErrorInfo || rspcErrorInfo.code === 500) return false;

			const { message } = rspcErrorInfo;

			if (message)
				form.setError(REMOTE_ERROR_FORM_FIELD, { type: 'remote', message: message });

			return true;
		},
		[form]
	);

	useCallbackToWatchForm(
		async (values) => {
			form.clearErrors(REMOTE_ERROR_FORM_FIELD);
			// Only validate with backend if the form is locally valid
			if (!form.formState.isValid) return;
			try {
				await addIndexerRules(values, true);
			} catch (error) {
				handleAddError(error);
			}
		},
		[form, addIndexerRules, handleAddError]
	);

	const indexRules = listIndexerRules.data;
	const {
		formState: { isSubmitting: isFormSubmitting, errors: formErrors }
	} = form;
	return (
		<>
			<Card className="mb-2 flex flex-wrap justify-evenly">
				{indexRules ? (
					indexRules.map((rule) => (
						<RuleButton
							key={rule.id}
							rule={rule}
							field={field}
							editable={editable}
							disabled={!field}
						/>
					))
				) : (
					<p className={clsx(listIndexerRules.isError && 'text-red-500')}>
						{listIndexerRules.isError
							? 'Error while retriving indexer rules'
							: 'No indexer rules available'}
					</p>
				)}
			</Card>

			{
				// Portal is required for Form because this component can be inside another form element
				createPortal(
					<Form
						id={formId}
						form={form}
						disabled={isFormSubmitting}
						onSubmit={form.handleSubmit(async (values) => {
							try {
								await addIndexerRules(values);
							} catch (error) {
								if (handleAddError(error)) {
									// Reset form to remove isSubmitting state
									form.reset(
										{},
										{ keepValues: true, keepErrors: true, keepIsValid: true }
									);
								} else {
									showAlertDialog({
										title: 'Error',
										value: String(error) || 'Failed to create new indexer rule'
									});
									return;
								}
							}
							form.reset();
							await listIndexerRules.refetch();
						})}
						className="hidden h-0 w-0"
					/>,
					document.body
				)
			}

			{editable && (
				<FormProvider {...form}>
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
								className={clsx(
									'ml-1 transition',
									showCreateNewRule && 'rotate-90'
								)}
							/>
						</Button>

						{showCreateNewRule && (
							<div className="px-4 pb-4 pt-2">
								<h3 className="w-full text-center text-sm font-semibold">Rules</h3>

								<Divider className="mb-2" />

								<Controller
									name="parameters"
									render={({ field }) => (
										<>
											<div
												className={clsx(
													formErrors.parameters &&
														'!ring-1 !ring-red-500',
													'grid space-y-1 rounded-md border border-app-line/60 bg-app-overlay p-2'
												)}
											>
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
																<InfoPill className="mr-2 p-0.5">
																	{kind}
																</InfoPill>

																<p className="p-0.5 text-sm font-semibold text-ink-dull">
																	{rule}
																</p>

																<div className="grow" />

																{/* <p className="mx-2 rounded-md border border-app-line/30 bg-app-overlay/80 py-1 px-2 text-center text-sm text-ink-dull">
																	{kind}
																</p> */}

																<Button
																	variant="gray"
																	onClick={() =>
																		removeParameter(
																			field,
																			index
																		)
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

											<ErrorMessage name="parameters" className="mt-1" />

											<Tabs.Root
												value={currentTab}
												onValueChange={(tab) =>
													isKeyOf(RuleTabsInput, tab) &&
													setCurrentTab(tab)
												}
											>
												<Tabs.List className="flex flex-row">
													{Object.keys(RuleTabsInput).map((name) => (
														<Tabs.Trigger
															className="flex-auto !rounded-md py-2 text-sm font-medium"
															key={name}
															value={name}
														>
															{name}
														</Tabs.Trigger>
													))}
												</Tabs.List>

												{(Object.keys(RuleTabsInput) as RuleType[]).map(
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
										<FormInput
											size="md"
											form={formId}
											placeholder="Name"
											{...form.register('name')}
										/>

										<div className="mt-2 flex w-full flex-row">
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
															// TODO: This rule kinds are broken right now in the backend and this UI doesn't make much sense for them
															// kind.AcceptIfChildrenDirectoriesArePresent
															// kind.RejectIfChildrenDirectoriesArePresent
															const kind = ruleKindEnum.enum;
															field.onChange(
																checked
																	? kind.AcceptFilesByGlob
																	: kind.RejectFilesByGlob
															);
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
										form={formId}
										variant={isFormSubmitting ? 'outline' : 'accent'}
										className={inputSizes.md}
									>
										<Plus />
									</Button>
								</div>

								<ErrorMessage
									name={REMOTE_ERROR_FORM_FIELD}
									variant="large"
									className="mt-2"
								/>
							</div>
						)}
					</div>
				</FormProvider>
			)}
		</>
	);
}
