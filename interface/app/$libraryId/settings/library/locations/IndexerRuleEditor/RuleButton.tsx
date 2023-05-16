import clsx from 'clsx';
import { Trash } from 'phosphor-react';
import { MouseEventHandler, useState } from 'react';
import { IndexerRule, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button } from '@sd/ui';
import { showAlertDialog } from '~/components';
import { IndexerRuleIdFieldType } from '.';

interface RuleButtonProps<T extends IndexerRuleIdFieldType> {
	rule: IndexerRule;
	field?: T;
	disabled?: boolean;
}

function RuleButton<T extends IndexerRuleIdFieldType>({
	rule,
	field,
	disabled
}: RuleButtonProps<T>) {
	const listIndexerRules = useLibraryQuery(['locations.indexer_rules.list']);
	const deleteIndexerRule = useLibraryMutation(['locations.indexer_rules.delete']);
	const [isDeleting, setIsDeleting] = useState(false);
	const value = field?.value ?? [];
	const ruleEnabled = value.includes(rule.id);

	const ruleDeleteHandler = async () => {
		setIsDeleting(true);
		try {
			await deleteIndexerRule.mutateAsync(rule.id);
		} catch (error) {
			showAlertDialog({
				title: 'Error',
				value: String(error) || 'Failed to delete rule'
			});
		} finally {
			setIsDeleting(false);
		}

		await listIndexerRules.refetch();
	};

	const confirmDelete: MouseEventHandler<HTMLDivElement> = (e) => {
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
		<div
			className={
				'flex overflow-hidden rounded-md border-app-line bg-app-button hover:border-app-line'
			}
		>
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
				disabled={disabled || isDeleting || !field}
				className={
					'flex w-fit min-w-[130px] !cursor-pointer justify-between gap-2 overflow-hidden bg-transparent'
				}
			>
				{rule.name}
				<p className={clsx(`text-sm`, ruleEnabled ? 'text-ink-faint' : 'text-red-500')}>
					({ruleEnabled ? 'Enabled' : 'Disabled'})
				</p>
			</Button>
			{!rule.default && (
				<div
					onClick={confirmDelete}
					className={
						'flex h-full cursor-pointer items-center justify-center rounded-r bg-app-lightBox p-2 hover:brightness-105'
					}
				>
					<Trash />
				</div>
			)}
		</div>
	);
}

export default RuleButton;
