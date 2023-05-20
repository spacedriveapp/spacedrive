import clsx from 'clsx';
import { Trash } from 'phosphor-react';
import { Info } from 'phosphor-react';
import { ChangeEvent, useId } from 'react';
import { useCallback } from 'react';
import { createPortal } from 'react-dom';
import { Controller, FormProvider, useFieldArray } from 'react-hook-form';
import { z } from 'zod';
import { RuleKind, UnionToTuple, extractInfoRSPCError, useLibraryMutation } from '@sd/client';
import { IndexerRuleCreateArgs } from '@sd/client';
import { Button, Card, Divider, Input, Select, SelectOption, Switch, Tooltip } from '@sd/ui';
import { ErrorMessage, Form, useZodForm } from '@sd/ui/src/forms';
import { InputKinds, RuleInput, validateInput } from './RuleInput';

const ruleKinds: UnionToTuple<RuleKind> = [
	'AcceptFilesByGlob',
	'RejectFilesByGlob',
	'AcceptIfChildrenDirectoriesArePresent',
	'RejectIfChildrenDirectoriesArePresent'
];
const ruleKindEnum = z.enum(ruleKinds);

const schema = z.object({
	name: z.string().min(3),
	kind: ruleKindEnum,
	rules: z.array(
		z.object({
			type: z.string(),
			value: z.string().min(1, { message: 'Value required' })
		})
	)
});

type formType = z.infer<typeof schema>;

interface Props {
	onSubmitted?: () => void;
}

const RulesForm = ({ onSubmitted }: Props) => {
	const selectValues = ['Name', 'Extension', 'Path', 'Advanced'];
	const REMOTE_ERROR_FORM_FIELD = 'root.serverError';
	const createIndexerRules = useLibraryMutation(['locations.indexer_rules.create']);
	const formId = useId();
	const form = useZodForm({
		schema,
		mode: 'onBlur',
		reValidateMode: 'onBlur',
		defaultValues: {
			name: '',
			kind: 'RejectFilesByGlob',
			rules: [
				{
					type: selectValues[0],
					value: ''
				}
			]
		}
	});
	const errors = form.formState.errors;

	const { fields, append, remove } = useFieldArray({
		control: form.control,
		name: 'rules'
	});

	//this is used to update the input type based on rule 'type' selected
	const memoWatchRules = useCallback(
		(index: number) => {
			return form.watch(`rules.${index}.type`) as InputKinds;
		},
		[form]
	);

	const inputValidator = (
		index: number,
		watcher: InputKinds,
		e: ChangeEvent<HTMLInputElement>
	) => {
		const isValid = validateInput(watcher, e.target.value);
		if (!isValid?.value) {
			form.setError(`rules.${index}.value`, {
				message: isValid?.message
			});
		} else {
			form.clearErrors(`rules.${index}.value`);
		}
	};

	const addIndexerRules = form.handleSubmit(async (data: formType) => {
		const formatData = {
			kind: data.kind,
			name: data.name,
			dry_run: false,
			parameters: data.rules.flatMap(({ type, value }) => {
				switch (type) {
					case 'Name':
						return `**/${value}`;
					case 'Extension':
						// .tar should work for .tar.gz, .tar.bz2, etc.
						return [`**/*${value}`, `**/*${value}.*`];
					default:
						return value;
				}
			})
		} as IndexerRuleCreateArgs;

		try {
			await createIndexerRules.mutateAsync(formatData);
		} catch (error) {
			const rspcErrorInfo = extractInfoRSPCError(error);
			if (!rspcErrorInfo || rspcErrorInfo.code === 500) return false;

			const { message } = rspcErrorInfo;

			if (message)
				form.setError(REMOTE_ERROR_FORM_FIELD, { type: 'remote', message: message });
		}
	});

	if (form.formState.isSubmitSuccessful) onSubmitted?.();

	return (
		// The portal is required for Form because this component can be nested inside another form element
		<>
			{createPortal(
				<Form id={formId} form={form} onSubmit={addIndexerRules} />,
				document.body
			)}
			<FormProvider {...form}>
				<h3 className="mb-[15px] w-full text-sm font-semibold">Name</h3>
				<Input
					className={errors.name && 'border border-red-500'}
					form={formId}
					size="md"
					placeholder="Name"
					{...form.register('name')}
				/>
				{errors.name && <p className="mt-2 text-sm text-red-500">{errors.name?.message}</p>}
				<h3 className="mb-[15px] mt-[20px] w-full text-sm font-semibold">Rules</h3>
				<div
					className={
						'grid space-y-1 rounded-md border border-app-line/60 bg-app-input p-2 pb-0'
					}
				>
					<div className="mb-4 grid grid-cols-3 px-3 pt-4 text-sm font-bold">
						<h3 className="pl-2">Type</h3>
						<h3 className="pl-2">Value</h3>
					</div>
					{fields.map((field, index) => {
						return (
							<Card
								className="grid w-full grid-cols-3 gap-3 border-app-line p-0 !px-2 hover:bg-app-box/70"
								key={field.id}
							>
								<Controller
									name={`rules.${index}.type` as const}
									control={form.control}
									render={({ field }) => (
										<Select
											{...field}
											className="w-full"
											onChange={(value) => {
												field.onChange(value);
												form.resetField(`rules.${index}.value`);
											}}
										>
											{selectValues.map((value) => (
												<SelectOption key={value} value={value}>
													{value}
												</SelectOption>
											))}
										</Select>
									)}
								/>
								<Controller
									name={`rules.${index}.value` as const}
									control={form.control}
									render={({ field }) => {
										return (
											<div className="flex flex-col">
												<RuleInput
													className={clsx(
														'!h-[30px]',
														errors.rules?.[index]?.value &&
															'border border-red-500'
													)}
													kind={memoWatchRules(index) as InputKinds}
													{...field}
													onChange={(e) => {
														field.onChange(e.target.value);
														inputValidator(
															index,
															memoWatchRules(index),
															e
														);
													}}
													onBlur={(e) => {
														inputValidator(
															index,
															memoWatchRules(index),
															e
														);
													}}
												/>
												{errors.rules?.[index]?.value && (
													<span className="mt-2 text-red-500">
														{errors.rules[index]?.value?.message}
													</span>
												)}
											</div>
										);
									}}
								/>

								{index !== 0 && (
									<Button
										className="flex h-[32px] w-[32px] items-center
														 justify-self-end"
										variant="gray"
										onClick={() => remove(index)}
									>
										<Tooltip label="Delete rule">
											<Trash size={14} />
										</Tooltip>
									</Button>
								)}
							</Card>
						);
					})}
					<Button
						onClick={() =>
							append(
								{ type: selectValues[0] as string, value: '' },
								{ shouldFocus: false }
							)
						}
						className="!my-2 mx-auto w-full border
										!border-app-line !bg-app-darkBox py-2 !font-bold
										 hover:brightness-105"
					>
						+
					</Button>
				</div>
				<Divider className="my-[25px]" />
				<div className="flex w-full justify-center">
					<div className="mb-5 flex items-center gap-2">
						<p className="text-sm text-ink-faint">Blacklist</p>
						<Controller
							name="kind"
							render={({ field }) => (
								<Switch
									onCheckedChange={(checked) => {
										// TODO: These rule kinds are broken right now in the backend and this UI doesn't make much sense for them
										// kind.AcceptIfChildrenDirectoriesArePresent
										// kind.RejectIfChildrenDirectoriesArePresent
										const kind = ruleKindEnum.enum;
										field.onChange(
											checked
												? kind.AcceptFilesByGlob
												: kind.RejectFilesByGlob
										);
									}}
									size="md"
								/>
							)}
							control={form.control}
						/>
						<p className="text-sm text-ink-faint">Whitelist</p>
						<Tooltip label="By default, an indexer rule acts as a deny list, causing a location to ignore any file that match its rules. Enabling this will make it act as an allow list, and the location will only display files that match its rules.">
							<Info />
						</Tooltip>
					</div>
				</div>
				<Button form={formId} type="submit" variant="accent" className="mx-auto w-[90px]">
					Save
				</Button>
				<div className="text-center">
					<ErrorMessage name={REMOTE_ERROR_FORM_FIELD} variant="large" className="mt-2" />
				</div>
			</FormProvider>
		</>
	);
};

export default RulesForm;
