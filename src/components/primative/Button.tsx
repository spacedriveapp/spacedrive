import React from 'react';
import { ButtonHTMLAttributes, useState } from 'react';
import { Switch } from '@headlessui/react';
import clsx from 'clsx';

const sizes = {
  default: 'py-1 px-3 text-md font-medium',
  sm: 'py-1 px-2 text-sm font-medium'
};

const variants = {
  default: `
    bg-gray-50 
    shadow-sm
    hover:bg-gray-100 
    active:bg-gray-50  
    dark:bg-gray-800
    dark:hover:bg-gray-700
    dark:active:bg-gray-600
    
    border-gray-100 
    hover:border-gray-200
    active:border-gray-100
    dark:border-gray-700 
    dark:hover:border-gray-600

    text-gray-700
    hover:text-gray-900 
    active:text-gray-600 
    dark:text-gray-200  
    dark:active:text-white 
    dark:hover:text-white 
    
  `,
  primary:
    'bg-primary shadow-sm border-primary-600 dark:border-primary-400 active:bg-primary-600 active:border-primary-700 hover:bg-primary-400 hover:border-primary-500'
};

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: keyof typeof variants;
  size?: keyof typeof sizes;
  loading?: boolean;
}

export const Button: React.FC<ButtonProps> = ({ loading, ...props }) => {
  return (
    <button
      {...props}
      className={clsx(
        'flex justify-center  border rounded-md transition-all duration-100 text-white',
        { 'opacity-5': loading },
        sizes[props.size || 'default'],
        variants[props.variant || 'default'],
        props.className
      )}
    >
      {props.children}
    </button>
  );
};
