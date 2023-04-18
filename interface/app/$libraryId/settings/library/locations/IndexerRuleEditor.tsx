// import { PlusSquare } from '@phosphor-icons/react';
import clsx from 'clsx';
import { ControllerRenderProps, FieldPath } from 'react-hook-form';
import { useLibraryQuery } from '@sd/client';
import { Button, Card } from '@sd/ui';

interface FormFields {
	indexerRulesIds: number[];
}

type FieldType = ControllerRenderProps<
	FormFields,
	Exclude<FieldPath<FormFields>, `indexerRulesIds.${number}`>
>;

export interface IndexerRuleEditorProps<T extends FieldType> {
	field: T;
	editable?: boolean;
}

export function IndexerRuleEditor<T extends FieldType>({
	field,
	editable
}: IndexerRuleEditorProps<T>) {
	const listIndexerRules = useLibraryQuery(['locations.indexer_rules.list']);
	const indexRules = listIndexerRules.data;
	return (
		<Card className="mb-2 flex flex-wrap justify-evenly">
			{indexRules ? (
				indexRules.map((rule) => {
					const { id, name } = rule;
					const enabled = field.value.includes(id);
					return (
						<Button
							key={id}
							size="sm"
							onClick={() =>
								field.onChange(
									enabled
										? field.value.filter((fieldValue) => fieldValue !== rule.id)
										: Array.from(new Set([...field.value, rule.id]))
								)
							}
							variant={enabled ? 'colored' : 'outline'}
							className={clsx('m-1 flex-auto', enabled && 'border-accent bg-accent')}
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
			{/* {editable && (
				<Button
					size="icon"
					onClick={() => console.log('TODO')}
					variant="outline"
					className="m-1 flex-[0_0_99%] text-center leading-none"
				>
					<PlusSquare weight="light" size={18} className="inline" />
				</Button>
			)} */}
		</Card>
	);
}
