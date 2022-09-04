import React from 'react';

interface MetaItemProps {
	title?: string;
	value: string | React.ReactNode;
}

export const MetaItem = (props: MetaItemProps) => {
	return (
		<div data-tip={props.value} className="flex flex-col px-4 py-1.5 meta-item">
			{!!props.title && <h5 className="text-xs font-bold">{props.title}</h5>}
			{typeof props.value === 'string' ? (
				<p className="text-xs text-gray-600 break-all truncate dark:text-gray-300">{props.value}</p>
			) : (
				props.value
			)}
		</div>
	);
};
