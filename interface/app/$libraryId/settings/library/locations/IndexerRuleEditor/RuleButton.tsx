import clsx from 'clsx';
import { X } from 'phosphor-react';
import { MouseEventHandler, useRef, useState } from 'react';
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
	const timeoutId = useRef<number>(0);
	const [willDelete, setWillDelete] = useState(false);
	const [isDeleting, setIsDeleting] = useState(false);
	const listIndexerRules = useLibraryQuery(['locations.indexer_rules.list']);
	const deleteIndexerRule = useLibraryMutation(['locations.indexer_rules.delete']);
	const [showDelete, setShowDelete] = useState(false);

	const value = field?.value ?? [];
	const ruleEnabled = value.includes(rule.id);

	const ruleDeleteHandler: MouseEventHandler<HTMLDivElement> = async (e) => {
		e.stopPropagation();
		e.preventDefault();
		if (willDelete) {
			setIsDeleting(true);

			try {
				await deleteIndexerRule.mutateAsync(rule.id);
			} catch (error) {
				showAlertDialog({
					title: 'Error',
					value: String(error) || 'Failed to add location'
				});
			} finally {
				setWillDelete(false);
				setIsDeleting(false);
			}

			await listIndexerRules.refetch();
		} else {
			setWillDelete(true);
		}
	};

	return (
		<Button
			size="sm"
			onMouseEnter={() => setShowDelete((t) => !t)}
			onMouseLeave={() => setShowDelete((t) => !t)}
			onClick={
				field &&
				(() =>
					field.onChange(
						ruleEnabled
							? value.filter((v) => v !== rule.id)
							: Array.from(new Set([...value, rule.id]))
					))
			}
			variant={disabled ? 'outline' : ruleEnabled ? 'gray' : 'colored'}
			disabled={disabled || isDeleting || !field}
			className={clsx(
				'relative flex w-fit min-w-[130px] justify-between gap-2 overflow-hidden'
			)}
		>
			{rule.name}
			<p className="text-sm text-ink-faint">({ruleEnabled ? 'Enabled' : 'Disabled'})</p>
			{!rule.default && (showDelete || willDelete) && (
				<div
					onClick={ruleDeleteHandler}
					onMouseEnter={() => {
						const id = timeoutId.current;
						timeoutId.current = 0;
						if (id) clearTimeout(id);
					}}
					onMouseLeave={() => {
						timeoutId.current = setTimeout(() => {
							timeoutId.current = 0;
							if (!isDeleting) setWillDelete(false);
						}, 500);
					}}
					className={clsx(
						'absolute right-0 top-0 flex h-full cursor-pointer content-center items-center justify-center justify-items-center overflow-hidden bg-red-500 transition-[width]',
						willDelete ? 'w-full' : 'w-6'
					)}
				>
					{willDelete ? 'Delete?' : <X className="!pointer-events-none" />}
				</div>
			)}
		</Button>
	);
}

export default RuleButton;
