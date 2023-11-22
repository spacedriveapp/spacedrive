import type { PropsWithChildren } from 'react';
import { CategoryHeading } from '@sd/ui';

export default (
	props: PropsWithChildren<{
		name: string;
		actionArea?: React.ReactNode;
		onDoubleClick?: () => void;
	}>
) => (
	<div onDoubleClick={props.onDoubleClick} className="group">
		<div className="mb-1 flex items-center justify-between">
			<CategoryHeading className="ml-1">{props.name}</CategoryHeading>
			{props.actionArea && (
				<div className="text-sidebar-inkFaint opacity-0 transition-all duration-300 hover:!opacity-100 group-hover:opacity-30">
					{props.actionArea}
				</div>
			)}
		</div>
		{props.children}
	</div>
);
