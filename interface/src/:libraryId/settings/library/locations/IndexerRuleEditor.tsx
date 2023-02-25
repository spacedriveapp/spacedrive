import { useLibraryQuery } from '@sd/client';
import { Card, Input, tw } from '@sd/ui';

interface Props {
	locationId: string;
}

export const Rule = tw.span`inline border border-transparent px-1 text-[11px] font-medium shadow shadow-app-shade/5 bg-app-selected rounded-md text-ink-dull`;

export function IndexerRuleEditor({ locationId }: Props) {
	const listIndexerRules = useLibraryQuery(['locations.indexer_rules.list'], {});
	const currentLocationIndexerRules = useLibraryQuery(
		['locations.indexer_rules.listForLocation', Number(locationId)],
		{}
	);

	return (
		<div className="flex flex-col">
			{/* <Input /> */}
			{/* <Card className="flex flex-wrap mb-2 space-x-1">
				{currentLocationIndexerRules.data?.map((rule) => (
					<Rule key={rule.indexer_rule.id}>{rule.indexer_rule.name}</Rule>
				))}
			</Card> */}
			<Card className="mb-2 flex flex-wrap space-x-1">
				{listIndexerRules.data?.map((rule) => (
					<Rule key={rule.id}>{rule.name}</Rule>
				))}
			</Card>
		</div>
	);
}
