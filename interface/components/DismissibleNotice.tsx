import clsx from 'clsx';
import { ReactNode } from 'react';
import { Button } from '@sd/ui';
import {
	dismissibleNoticeStore,
	getDismissibleNoticeStore,
	useDismissibleNoticeStore
} from '~/hooks/useDismissibleNoticeStore';

interface Props {
	icon?: ReactNode;
	title: string | ReactNode;
	description: string;
	onDismiss?: () => void;
	onLearnMore?: () => void;
	className?: string;
	storageKey: keyof typeof dismissibleNoticeStore;
}

export default (props: Props) => {
	const dismissibleNoticeStore = useDismissibleNoticeStore();

	if (dismissibleNoticeStore[props.storageKey]) return null;
	return (
		<div
			className={clsx(
				'rounded-md bg-gradient-to-l from-accent-deep via-accent-faint to-purple-500 p-1',
				props.className
			)}
		>
			<div className="flex items-center rounded bg-app px-3 py-4">
				{props.icon}

				<div className="flex flex-1 flex-col justify-center">
					<h1 className="text-xl font-bold text-ink">{props.title}</h1>
					<p className="text-xs text-ink-dull">{props.description}</p>
				</div>

				<div className="ml-6 mr-3 space-x-2">
					{props.onLearnMore && (
						<Button
							variant="outline"
							className="border-white/10 font-medium hover:border-white/20"
							onClick={props.onLearnMore}
						>
							Learn More
						</Button>
					)}
					<Button
						variant="accent"
						className="font-medium"
						onClick={() => {
							getDismissibleNoticeStore()[props.storageKey] = true;
							props.onDismiss?.();
						}}
					>
						Got it
					</Button>
				</div>
			</div>
		</div>
	);
};
