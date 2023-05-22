import clsx from 'clsx';
import { useRef } from 'react';
import { IndexerRule } from '@sd/client';
import { InfoPill } from '~/app/$libraryId/Explorer/Inspector';
import { IndexerRuleIdFieldType } from '.';

interface RuleButtonProps<T extends IndexerRuleIdFieldType> {
	rule: IndexerRule;
	field?: T;
	onClick?: React.ComponentProps<'div'>['onClick'];
	className?: string;
}

function RuleButton<T extends IndexerRuleIdFieldType>({
	rule,
	field,
	onClick,
	className
}: RuleButtonProps<T>) {
	const value = field?.value ?? [];
	const toggleRef = useRef<HTMLElement>(null);
	const ruleEnabled = value.includes(rule.id);

	return (
		<div
			onClick={
				onClick ??
				(() => {
					if (toggleRef.current) toggleRef.current.click();
				})
			}
			className={clsx(
				`relative flex w-[100px] min-w-[150px] justify-between gap-2 rounded-md border border-app-line py-2`,
				className
			)}
		>
			<div className="w-full">
				<p className="mb-2 text-center text-sm">{rule.name}</p>
				<div className="flex flex-wrap justify-center gap-2">
					<InfoPill
						ref={toggleRef}
						onClick={
							field &&
							((e) => {
								e.stopPropagation();
								field.onChange(
									ruleEnabled
										? value.filter((v) => v !== rule.id)
										: Array.from(new Set([...value, rule.id]))
								);
							})
						}
						className={clsx(
							'hover:brightness-125',
							ruleEnabled ? '!text-green-500' : 'text-red-500'
						)}
					>
						{ruleEnabled ? 'Enabled' : 'Disabled'}
					</InfoPill>
					{rule.default && <InfoPill className="text-ink-faint">System</InfoPill>}
				</div>
			</div>
		</div>
	);
}

export default RuleButton;
