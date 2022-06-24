import clsx from 'clsx';
import React from 'react';

export interface CheckboxProps extends React.InputHTMLAttributes<HTMLInputElement> {
	containerClasname?: string;
}

export const Checkbox: React.FC<CheckboxProps> = (props) => {
	return (
		<label
			className={clsx(
				'flex items-center text-sm font-medium text-gray-700 dark:text-gray-100',
				props.containerClasname
			)}
		>
			<input
				{...props}
				type="checkbox"
				className={clsx(
					`
        bg-gray-50
        hover:bg-gray-100
        dark:bg-gray-800
        border-gray-100
        hover:border-gray-200
        dark:border-gray-700
        dark:hover:bg-gray-700
        dark:hover:border-gray-600
        transition 
        rounded 
        mr-2
        text-primary 
        checked:ring-2 checked:ring-primary-500
        `,
					props.className
				)}
			/>
			<span className="select-none">Checkbox</span>
		</label>
	);
};
