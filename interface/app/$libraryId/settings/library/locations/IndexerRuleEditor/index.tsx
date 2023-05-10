import clsx from 'clsx';
import { Dispatch, SetStateAction } from 'react';
import { ControllerRenderProps } from 'react-hook-form';
import { useLibraryQuery } from '@sd/client';
import { Divider } from '@sd/ui';
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

	return (
		<>
			<div className="mb-[25px] flex flex-wrap gap-1">
				{indexRules ? (
					indexRules.map((rule) => (
						<RuleButton key={rule.id} rule={rule} field={field} disabled={!field} />
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
					<Divider className="mb-[25px]" />
					<RulesForm setToggleNewRule={setToggleNewRule} />
				</>
			)}
		</>
	);
}
