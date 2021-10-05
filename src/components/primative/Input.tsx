import clsx from 'clsx';
import React from 'react';

const variants = {
  default:
    'border-gray-200 dark:border-gray-700 dark:bg-gray-800 dark:hover:bg-gray-700 dark:text-white dark:focus:hover:bg-gray-800 focus:border-gray-300 placeholder-gray-300 focus:ring-gray-200 dark:focus:border-gray-900 dark:focus:ring-gray-600',
  primary: ''
};

interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  variant?: keyof typeof variants;
}

export const Input = (props: InputProps) => {
  return (
    <input
      {...props}
      className={clsx(
        `px-3 py-1 rounded-md border leading-7 outline-none shadow-xs focus:ring-2 transition-all`,
        variants[props.variant || 'default'],
        props.className
      )}
    />
  );
};

export const Label: React.FC<{ slug?: string }> = (props) => (
  <label className="text-sm font-bold" htmlFor={props.slug}>
    {props.children}
  </label>
);
