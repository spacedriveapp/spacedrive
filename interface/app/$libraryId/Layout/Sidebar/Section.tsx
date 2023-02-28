import { PropsWithChildren } from 'react';
import { CategoryHeading } from '@sd/ui';

export default (
	props: PropsWithChildren<{
		name: string;
		actionArea?: React.ReactNode;
	}>
) => (
	<div className="group mt-5">
		<div className="mb-1 flex items-center justify-between">
			<CategoryHeading className="ml-1">{props.name}</CategoryHeading>
			<div className="text-ink-faint opacity-0 transition-all duration-300 hover:!opacity-100 group-hover:opacity-30">
				{props.actionArea}
			</div>
		</div>
		{props.children}
	</div>
);
