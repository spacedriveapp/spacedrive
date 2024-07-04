import { Info, Trash } from '@phosphor-icons/react';
import clsx from 'clsx';
import { ChangeEvent, useCallback, useEffect, useId } from 'react';
import { createPortal } from 'react-dom';
import { Controller, FormProvider, useFieldArray } from 'react-hook-form';
import {
	extractInfoRSPCError,
	IndexerRuleCreateArgs,
	RuleKind,
	UnionToTuple,
	useLibraryMutation,
	useZodForm
} from '@sd/client';
import { Button, Card, Divider, Input, Select, SelectOption, Tooltip } from '@sd/ui';
import { ErrorMessage, Form, z } from '@sd/ui/src/forms';
import { useLocale } from '~/hooks';

import { InputKinds, RuleInput, validateInput } from './RuleInput';

const ruleKinds: UnionToTuple<RuleKind> = [
	'AcceptFilesByGlob',
	'RejectFilesByGlob',
	'AcceptIfChildrenDirectoriesArePresent',
	'RejectIfChildrenDirectoriesArePresent',
	'IgnoredByGit'
];
const ruleKindEnum = z.enum(ruleKinds);

const schema = z.object({
	name: z.string().trim().min(3).max(18),
	rules: z.array(
		z.object({
			type: z.string(),
			value: z.string().min(1, { message: 'Value required' }),
			kind: ruleKindEnum
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
	const { t } = useLocale();
	const modeOptions: { value: RuleKind; label: string }[] = [
		{ value: 'RejectFilesByGlob', label: t('reject_files') },
		{ value: 'AcceptFilesByGlob', label: t('accept_files') }
	];
	const form = useZodForm({
		schema,
		mode: 'onBlur',
		reValidateMode: 'onBlur',
		defaultValues: {
			name: '',
			rules: [
				{
					type: selectValues[0],
					value: '',
					kind: modeOptions[0]?.value
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
			name: data.name,
			dry_run: false,
			rules: data.rules.map(({ type, value, kind }) => {
				switch (type) {
					case 'Name':
						return [kind, [`**/${value}`]];
					case 'Extension':
						// .tar should work for .tar.gz, .tar.bz2, etc.
						return [kind, [`**/*${value}`, `**/*${value}.*`]];
					default:
						return [kind, [value]];
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

	useEffect(() => {
		if (form.formState.isSubmitSuccessful) onSubmitted?.();
	}, [form.formState.isSubmitSuccessful, onSubmitted]);

	return (
		// The portal is required for Form because this component can be nested inside another form element
		<>
			{createPortal(
				<Form id={formId} form={form} onSubmit={addIndexerRules} />,
				document.body
			)}
			<FormProvider {...form}>
				<h3 className="mb-[15px] w-full text-sm font-semibold">{t('name')}</h3>
				<Input
					className={errors.name && 'border border-red-500'}
					form={formId}
					size="md"
					placeholder={t('name')}
					maxLength={18}
					{...form.register('name')}
				/>
				{errors.name && <p className="mt-2 text-sm text-red-500">{errors.name?.message}</p>}
				<h3 className="mb-[15px] mt-[20px] w-full text-sm font-semibold">{t('rules')}</h3>
				<div
					className={
						'grid space-y-1 rounded-md border border-app-line/60 bg-app-input p-2'
					}
				>
					<div className="mb-2 grid w-full grid-cols-4 items-center pt-2 text-center text-[11px] font-bold">
						<h3>{t('type')}</h3>
						<h3>{t('value')}</h3>
						<h3 className="flex items-center justify-center gap-1">
							{t('mode')}
							<Tooltip label={t('indexer_rule_reject_allow_label')}>
								<Info />
							</Tooltip>
						</h3>
					</div>
					{fields.map((field, index) => {
						return (
							<Card
								className="grid  w-full grid-cols-4 gap-3 border-app-line p-0 !px-2 hover:bg-app-box/70"
								key={field.id}
							>
								<Controller
									name={`rules.${index}.type` as const}
									control={form.control}
									render={({ field }) => (
										<Select
											{...field}
											className="!h-[30px] w-full"
											onChange={(value) => {
												field.onChange(value);
												form.resetField(`rules.${index}.value`);
											}}
										>
											{selectValues.map((value) => (
												<SelectOption key={value} value={value}>
													{t(`${value.toLowerCase()}`)}
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
											<div className="flex w-full flex-col">
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
								<Controller
									name={`rules.${index}.kind` as const}
									render={({ field }) => (
										<Select
											{...field}
											className="!h-[30px] w-full"
											onChange={(value) => {
												field.onChange(value);
											}}
										>
											{modeOptions.map(({ label, value }) => (
												<SelectOption key={value} value={value}>
													{label}
												</SelectOption>
											))}
										</Select>
									)}
									control={form.control}
								/>
								{index !== 0 && (
									<Button
										className="flex size-[32px] items-center justify-self-end"
										variant="gray"
										onClick={() => remove(index)}
									>
										<Tooltip label={t('delete_rule')}>
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
								{
									type: selectValues[0] as string,
									value: '',
									kind: 'RejectFilesByGlob'
								},
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
				<Button form={formId} type="submit" variant="accent" className="mx-auto w-[90px]">
					{t('save')}
				</Button>
				<div className="text-center">
					<ErrorMessage name={REMOTE_ERROR_FORM_FIELD} variant="large" className="mt-2" />
				</div>
			</FormProvider>
		</>
	);
};

export default RulesForm;
