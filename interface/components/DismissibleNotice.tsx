import clsx from 'clsx';
import { ReactNode } from 'react';
import { Button } from '@sd/ui';
import { useLocale } from '~/hooks';
import {
	dismissibleNoticeStore,
	getDismissibleNoticeStore,
	useDismissibleNoticeStore
} from '~/hooks/useDismissibleNoticeStore';

interface Props extends Omit<React.HTMLAttributes<HTMLDivElement>, 'title'> {
	icon?: ReactNode;
	title: string | ReactNode;
	description: string;
	onDismiss?: () => void;
	onLearnMore?: () => void;
	className?: string;
	storageKey: keyof typeof dismissibleNoticeStore;
}

export default ({
	icon,
	title,
	description,
	onDismiss,
	onLearnMore,
	storageKey,
	className,
	...props
}: Props) => {
	const dismissibleNoticeStore = useDismissibleNoticeStore();
	const { t } = useLocale();

	if (dismissibleNoticeStore[storageKey]) return null;

	return (
		<div
			className={clsx(
				'rounded-md bg-gradient-to-l from-accent-deep via-accent-faint to-purple-500 p-1',
				className
			)}
			{...props}
		>
			<div className="flex items-center rounded bg-app px-3 py-4">
				{icon}

				<div className="flex flex-1 flex-col justify-center">
					<h1 className="text-xl font-bold text-ink">{title}</h1>
					<p className="text-xs text-ink-dull">{description}</p>
				</div>

				<div className="ml-6 mr-3 space-x-2">
					{onLearnMore && (
						<Button
							variant="outline"
							className="border-white/10 font-medium hover:border-white/20"
							onClick={onLearnMore}
						>
							{t('learn_more')}
						</Button>
					)}
					<Button
						variant="accent"
						className="font-medium"
						onClick={() => {
							getDismissibleNoticeStore()[storageKey] = true;
							onDismiss?.();
						}}
					>
						{t('got_it')}
					</Button>
				</div>
			</div>
		</div>
	);
};
