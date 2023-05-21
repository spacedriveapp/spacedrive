import clsx from 'clsx';
import { Trash } from 'phosphor-react';
import { MouseEventHandler, useState } from 'react';
import { ControllerRenderProps } from 'react-hook-form';
import { IndexerRule, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, Divider, Label } from '@sd/ui';
import { InfoText } from '@sd/ui/src/forms';
import { showAlertDialog } from '~/components';
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
}

export default function IndexerRuleEditor<T extends IndexerRuleIdFieldType>({
	infoText,
	editable,
	...props
}: IndexerRuleEditorProps<T>) {
	const listIndexerRules = useLibraryQuery(['locations.indexer_rules.list']);
	const indexRules = listIndexerRules.data;
	const [isDeleting, setIsDeleting] = useState(false);
	const [selectedRule, setSelectedRule] = useState<IndexerRule | undefined>(undefined);
	const [toggleNewRule, setToggleNewRule] = useState(false);
	const deleteIndexerRule = useLibraryMutation(['locations.indexer_rules.delete']);

	const deleteRule: MouseEventHandler<HTMLButtonElement> = (e) => {
		if (!selectedRule) return;

		showAlertDialog({
			title: 'Delete',
			value: 'Are you sure you want to delete this rule?',
			label: 'Confirm',
			onSubmit: async () => {
				setIsDeleting(true);

				try {
					await deleteIndexerRule.mutateAsync(selectedRule.id);
				} catch (error) {
					showAlertDialog({
						title: 'Error',
						value: String(error) || 'Failed to delete rule'
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
				<div className="grow">
					<Label className="mb-2">{props.label || 'Indexer rules'}</Label>
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
							<Trash className="-mt-0.5 mr-1.5 inline h-4 w-4" />
							Delete
						</Button>
						<Button
							size="sm"
							variant="accent"
							onClick={() => setToggleNewRule(!toggleNewRule)}
							className={clsx('px-5', toggleNewRule && 'opacity-50')}
						>
							New
						</Button>
					</>
				)}
			</div>

			<div className="flex flex-wrap justify-center gap-1">
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
								selectedRule?.id === rule.id ? 'bg-app-darkBox' : 'bg-app-input'
							)}
						/>
					))
				) : (
					<p className={clsx(listIndexerRules.isError && 'text-red-500')}>
						{listIndexerRules.isError
							? 'Error while retriving indexer rules'
							: 'No indexer rules available'}
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
