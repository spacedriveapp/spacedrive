import { Trash } from '@phosphor-icons/react';
import clsx from 'clsx';
import { MouseEventHandler, useState } from 'react';
import { ControllerRenderProps } from 'react-hook-form';
import { IndexerRule, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, Divider, Label, toast } from '@sd/ui';
import { InfoText } from '@sd/ui/src/forms';
import { showAlertDialog } from '~/components';
import { useLocale } from '~/hooks';

import RuleButton from './RuleButton';
import RulesForm from './RulesForm';

export type IndexerRuleIdFieldType = ControllerRenderProps<
	{ indexerRulesIds: number[] },
	'indexerRulesIds'
>;

export interface IndexerRuleEditorProps<T extends IndexerRuleIdFieldType> {
	label?: string;
	field?: T;
	infoText?: string;
	editable?: boolean;
	className?: string;
	ruleButtonClass?: string;
	rulesContainerClass?: string;
}

export default function IndexerRuleEditor<T extends IndexerRuleIdFieldType>({
	infoText,
	editable,
	ruleButtonClass,
	rulesContainerClass,
	...props
}: IndexerRuleEditorProps<T>) {
	const listIndexerRules = useLibraryQuery(['locations.indexer_rules.list']);
	const indexRules = listIndexerRules.data;
	const [isDeleting, setIsDeleting] = useState(false);
	const [selectedRule, setSelectedRule] = useState<IndexerRule | undefined>(undefined);
	const [toggleNewRule, setToggleNewRule] = useState(false);
	const deleteIndexerRule = useLibraryMutation(['locations.indexer_rules.delete']);

	const { t } = useLocale();

	const deleteRule: MouseEventHandler<HTMLButtonElement> = () => {
		if (!selectedRule) return;

		showAlertDialog({
			title: t('delete'),
			value: t('delete_rule_confirmation'),
			label: t('confirm'),
			cancelBtn: true,
			onSubmit: async () => {
				setIsDeleting(true);
				try {
					await deleteIndexerRule.mutateAsync(selectedRule.id);
				} catch (error) {
					toast.error({
						title: t('failed_to_delete_rule'),
						body: t('error_message', { error })
					});
				} finally {
					setIsDeleting(false);
					setSelectedRule(undefined);
				}

				await listIndexerRules.refetch();
			}
		});
	};

	const disableDelete = isDeleting || !selectedRule;
	return (
		<div className={props.className} onClick={() => setSelectedRule(undefined)}>
			<div className={'flex items-start justify-between'}>
				<div className="mb-1 grow">
					<Label>{props.label || t('indexer_rules')}</Label>
					{infoText && <InfoText className="mb-4">{infoText}</InfoText>}
				</div>
				{editable && (
					<>
						<Button
							size="sm"
							variant={disableDelete ? 'gray' : 'colored'}
							onClick={deleteRule}
							disabled={disableDelete}
							className={clsx(
								'mr-2 px-5',
								disableDelete || 'border-red-500 bg-red-500'
							)}
						>
							<Trash className="-mt-0.5 mr-1.5 inline size-4" />
							{t('delete')}
						</Button>
						<Button
							size="sm"
							variant="accent"
							onClick={() => setToggleNewRule(!toggleNewRule)}
							className={clsx('px-5', toggleNewRule && 'opacity-50')}
						>
							{t('new')}
						</Button>
					</>
				)}
			</div>

			<div className={clsx(rulesContainerClass, 'flex flex-wrap gap-1')}>
				{indexRules ? (
					indexRules.map((rule) => (
						<RuleButton
							key={rule.id}
							rule={rule}
							field={props.field}
							onClick={
								editable
									? (e) => {
											e.stopPropagation();
											if (!rule.default)
												setSelectedRule(
													selectedRule === rule ? undefined : rule
												);
										}
									: undefined
							}
							className={clsx(
								!(editable && rule.default) && 'cursor-pointer',
								editable || 'select-none',
								selectedRule?.id === rule.id ? 'bg-app-darkBox' : 'bg-app-input',
								ruleButtonClass
							)}
						/>
					))
				) : (
					<p className={clsx(listIndexerRules.isError && 'text-red-500')}>
						{listIndexerRules.isError
							? `${t('indexer_rules_error')}`
							: `${t('indexer_rules_not_available')}`}
					</p>
				)}
			</div>

			{editable && toggleNewRule && (
				<>
					<Divider className="my-[25px]" />
					<RulesForm onSubmitted={() => setToggleNewRule(false)} />
				</>
			)}
		</div>
	);
}
