import clsx from 'clsx';
import React from 'react';

const variants = {
  default: `
    shadow-sm
    bg-white
    hover:bg-white
    focus:hover:bg-white
    focus:bg-white
    dark:bg-gray-550
    dark:hover:bg-gray-550
    dark:focus:bg-gray-800
    dark:focus:hover:bg-gray-800

    border-gray-100
    hover:border-gray-200
    focus:border-white
    dark:border-gray-500
    dark:hover:border-gray-500
    dark:focus:border-gray-900

    focus:ring-primary-100 
    dark:focus:ring-gray-550

    dark:text-white 
    placeholder-gray-300
  `,
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
