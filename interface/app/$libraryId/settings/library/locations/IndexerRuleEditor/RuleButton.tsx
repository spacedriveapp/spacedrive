import clsx from 'clsx';
import { useRef } from 'react';
import { IndexerRule } from '@sd/client';
import { InfoPill } from '~/app/$libraryId/Explorer/Inspector';
import { useLocale } from '~/hooks';

import { IndexerRuleIdFieldType } from '.';

function ruleIsSystem(rule: IndexerRule) {
	const num = rule.pub_id?.[15 - 3];
	return num !== undefined ? num === 0 : false;
}

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
	const { t } = useLocale();

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
				<p className="mb-2 truncate px-2 text-center text-sm">
					{t(`${rule.name?.toLowerCase().split(' ').join('_')}`)}
				</p>
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
							'px-2 hover:brightness-110',
							ruleEnabled ? '!bg-accent !text-white' : 'text-ink'
						)}
					>
						{ruleEnabled ? t('enabled') : t('disabled')}
					</InfoPill>
					{ruleIsSystem(rule) && (
						<InfoPill className="px-2 text-ink-faint">{t('system')}</InfoPill>
					)}
				</div>
			</div>
		</div>
	);
}

export default RuleButton;
