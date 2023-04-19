import clsx from 'clsx';
import { CaretRight, Plus, Trash } from 'phosphor-react';
import { Ref, createRef, forwardRef, useId, useState } from 'react';
import { createPortal } from 'react-dom';
import { Controller, ControllerRenderProps, FieldPath } from 'react-hook-form';
import { RuleKind, UnionToTuple, isKeyOf, useLibraryMutation, useLibraryQuery } from '@sd/client';
import {
	Button,
	Card,
	CheckBox,
	Divider,
	Input,
	RadixCheckbox,
	Tabs,
	Tooltip,
	forms
} from '@sd/ui';
import { showAlertDialog } from '~/components/AlertDialog';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';
import { usePlatform } from '~/util/Platform';
import { openDirectoryPickerDialog } from './AddLocationDialog';

interface FormFields {
	indexerRulesIds: number[];
}

type FieldType = ControllerRenderProps<
	FormFields,
	Exclude<FieldPath<FormFields>, `indexerRulesIds.${number}`>
>;

export interface IndexerRuleEditorProps<T extends FieldType> {
	field?: T;
	editable?: boolean;
}

interface TabInputProps {
	form: string;
	className: string;
}

const { z, Form, useZodForm } = forms;

// NOTE: This should be updated whenever RuleKind is changed
const ruleKinds: UnionToTuple<RuleKind> = [
	'AcceptFilesByGlob',
	'RejectFilesByGlob',
	'AcceptIfChildrenDirectoriesArePresent',
	'RejectIfChildrenDirectoriesArePresent'
];

const ruleKindEnum = z.enum(ruleKinds);

const newRuleSchema = z.object({
	kind: ruleKindEnum,
	name: z.string(),
	parameters: z.array(z.string())
});

export function IndexerRuleEditor<T extends FieldType>({
	field,
	editable
}: IndexerRuleEditorProps<T>) {
	const os = useOperatingSystem(true);
	const form = useZodForm({
		schema: newRuleSchema,
		defaultValues: { kind: 'RejectFilesByGlob', name: '', parameters: [] }
	});
	const formId = useId();
	const platform = usePlatform();
	const listIndexerRules = useLibraryQuery(['locations.indexer_rules.list']);
	const createIndexerRules = useLibraryMutation(['locations.indexer_rules.create']);
	const [currentTab, setCurrentTab] = useState<keyof typeof tabs>('Name');
	const [showCreateNewRule, setShowCreateNewRule] = useState(false);

	const tabs = {
		Name: forwardRef<HTMLInputElement, TabInputProps>((props, ref) => (
			<Input ref={ref} size="md" placeholder="File/Directory name" {...props} />
		)),
		Extension: forwardRef<HTMLInputElement, TabInputProps>((props, ref) => (
			<Input
				ref={ref}
				size="md"
				placeholder="File extension (e.g., .mp4, .jpg, .txt)"
				{...props}
			/>
		)),
		Path: forwardRef<HTMLInputElement, TabInputProps>(({ className, ...props }, ref) => (
			<Input
				ref={ref}
				size="md"
				readOnly={platform.platform !== 'web'}
				className={clsx(className, platform.platform === 'web' || 'cursor-pointer')}
				placeholder={
					'Path (e.g., ' +
					(os === 'windows'
						? 'C:\\Users\\john\\Downloads'
						: os === 'macOS'
						? '/Users/clara/Pictures'
						: '/home/emily/Documents') +
					')'
				}
				onClick={() =>
					openDirectoryPickerDialog(platform)
						.then((path) => path)
						.catch((error) =>
							showAlertDialog({
								title: 'Error',
								value: String(error)
							})
						)
				}
				{...props}
			/>
		)),
		Advanced: forwardRef<HTMLInputElement, TabInputProps>((props, ref) => (
			<Input ref={ref} size="md" placeholder="Glob (e.g., **/.git)" {...props} />
		))
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
			{createPortal(
				<Form id={formId} form={form} className="hidden h-0 w-0" />,
				document.body
			)}
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
						Create new rule
						<CaretRight
							weight="bold"
							className={clsx('ml-1 transition', showCreateNewRule && 'rotate-90')}
						/>
					</Button>

					{showCreateNewRule && (
						<div className="px-4 pb-4 pt-2">
							<Input
								size="md"
								form={formId}
								required
								className="mb-2 cursor-pointer"
								placeholder="Rule name"
								{...form.register('name')}
							/>
							<div className="flex items-center justify-start">
								<span className="mr-3 ml-0.5 text-sm font-bold">
									Rule is an allow list
								</span>
								<Controller
									name="kind"
									render={({ field }) => (
										<RadixCheckbox
											onCheckedChange={(status) => {
												let { value } = field;
												const checked =
													status !== 'indeterminate' && status;
												switch (value) {
													case ruleKindEnum.enum.RejectFilesByGlob:
													case ruleKindEnum.enum.AcceptFilesByGlob:
														value = checked
															? ruleKindEnum.enum.AcceptFilesByGlob
															: ruleKindEnum.enum.RejectFilesByGlob;
														break;
													case ruleKindEnum.enum
														.AcceptIfChildrenDirectoriesArePresent:
													case ruleKindEnum.enum
														.RejectIfChildrenDirectoriesArePresent:
														value = checked
															? ruleKindEnum.enum
																	.AcceptIfChildrenDirectoriesArePresent
															: ruleKindEnum.enum
																	.RejectIfChildrenDirectoriesArePresent;
														break;
												}
												field.onChange(value);
											}}
										/>
									)}
									control={form.control}
								/>
							</div>
							<Controller
								name="parameters"
								render={({ field }) => (
									<>
										<div className="grid space-y-2">
											{form.getValues().parameters.map((parameter, index) => (
												<Card key={index} className="hover:bg-app-box/70">
													<p className="text-sm font-semibold">
														{parameter}
													</p>
													<div className="flex grow" />
													<Button
														variant="gray"
														onClick={() =>
															field.onChange(
																field.value
																	.slice(0, index)
																	.concat(
																		field.value.slice(index + 1)
																	)
															)
														}
													>
														<Tooltip label="Delete rule">
															<Trash size={16} />
														</Tooltip>
													</Button>
												</Card>
											))}
										</div>
										<Tabs.Root
											value={currentTab}
											onValueChange={(tab) =>
												isKeyOf(tab, tabs) && setCurrentTab(tab)
											}
										>
											<Tabs.List className="flex flex-row">
												{Object.keys(tabs).map((name) => (
													<Tabs.Trigger
														className="flex-auto py-2 text-sm font-medium"
														key={name}
														value={name}
													>
														{name}
													</Tabs.Trigger>
												))}
											</Tabs.List>
											{...Object.entries(tabs).map(([name, TabInput]) => {
												const inputRef = createRef<HTMLInputElement>();

												return (
													<Tabs.Content key={name} asChild value={name}>
														<div className="flex flex-row justify-between pt-4">
															<TabInput
																ref={inputRef}
																form={formId}
																className="mr-2 flex-1"
															/>
															<Button
																onClick={() => {
																	const { current: input } =
																		inputRef;
																	if (!input) return;
																	field.onChange([
																		...field.value,
																		input.value
																	]);
																	input.value = '';
																}}
																variant="accent"
															>
																<Plus />
															</Button>
														</div>
													</Tabs.Content>
												);
											})}
										</Tabs.Root>
									</>
								)}
								control={form.control}
							/>
						</div>
					)}
				</div>
			)}
		</>
	);
}
