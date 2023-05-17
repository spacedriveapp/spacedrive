import clsx from 'clsx';
import { Trash } from 'phosphor-react';
import { Dispatch, MouseEventHandler, SetStateAction, useState } from 'react';
import { ControllerRenderProps } from 'react-hook-form';
import { IndexerRule, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, Divider } from '@sd/ui';
import { showAlertDialog } from '~/components';
import RuleButton from './RuleButton';
import RulesForm from './RulesForm';

export type IndexerRuleIdFieldType = ControllerRenderProps<
	{ indexerRulesIds: number[] },
	'indexerRulesIds'
>;

export interface IndexerRuleEditorProps<T extends IndexerRuleIdFieldType> {
	field?: T;
	toggleNewRule?: boolean;
	setToggleNewRule?: Dispatch<SetStateAction<boolean>>;
}

export default function IndexerRuleEditor<T extends IndexerRuleIdFieldType>({
	field,
	toggleNewRule,
	setToggleNewRule
}: IndexerRuleEditorProps<T>) {
	const listIndexerRules = useLibraryQuery(['locations.indexer_rules.list']);
	const indexRules = listIndexerRules.data;
	const [ruleSelected, setRuleSelected] = useState<IndexerRule | undefined>(undefined);
	const [isDeleting, setIsDeleting] = useState(false);
	const deleteIndexerRule = useLibraryMutation(['locations.indexer_rules.delete']);

	const ruleDeleteHandler = async () => {
		setIsDeleting(true);
		try {
			await deleteIndexerRule.mutateAsync(ruleSelected?.id as number);
		} catch (error) {
			showAlertDialog({
				title: 'Error',
				value: String(error) || 'Failed to delete rule'
			});
		} finally {
			setIsDeleting(false);
			setRuleSelected(undefined);
		}

		await listIndexerRules.refetch();
	};

	const confirmDelete: MouseEventHandler<HTMLButtonElement> = (e) => {
		e.stopPropagation();
		e.preventDefault();
		showAlertDialog({
			title: 'Delete',
			value: 'Are you sure you want to delete this rule?',
			label: 'Confirm',
			onSubmit: ruleDeleteHandler
		});
	};

	return (
		<>
			<div className="flex w-full flex-wrap gap-1">
				{indexRules ? (
					indexRules.map((rule) => (
						<RuleButton
							ruleSelected={ruleSelected}
							setRuleSelected={(v) => setRuleSelected(v)}
							key={rule.id}
							rule={rule}
							field={field}
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
			{toggleNewRule && (
				<>
					<Divider className="my-[25px]" />
					<RulesForm setToggleNewRule={setToggleNewRule} />
				</>
			)}
			{ruleSelected && (
				<Button
					disabled={isDeleting || !field}
					onClick={confirmDelete}
					size="sm"
					variant="colored"
					className="mx-auto mt-5 border-red-500 bg-red-500"
				>
					<Trash className="-mt-0.5 mr-1.5 inline h-4 w-4" />
					Delete
				</Button>
			)}
		</>
	);
}
