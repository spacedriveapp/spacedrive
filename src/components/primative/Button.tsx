import React from 'react';
import { ButtonHTMLAttributes, useState } from 'react';
import { Switch } from '@headlessui/react';
import clsx from 'clsx';

const variants = {
  default: `
    bg-gray-100 
    hover:bg-gray-200 
    active:bg-gray-200  
    dark:bg-gray-800
    dark:hover:bg-gray-700
    dark:active:bg-gray-600
    
    border-gray-200 
    hover:border-gray-300
    active:border-gray-200
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
    'bg-primary border-primary-600 dark:border-primary-400 active:bg-primary-600 active:border-primary-700 hover:bg-primary-400 hover:border-primary-500'
};

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: keyof typeof variants;
  loading?: boolean;
}

export const Button: React.FC<ButtonProps> = ({ loading, ...props }) => {
  return (
    <button
      {...props}
      className={clsx(
        'flex justify-center py-1 px-3 border rounded-md font-medium text-md transition-all duration-100 text-white',
        { 'opacity-5': loading },
        variants[props.variant || 'default'],
        props.className
      )}
    >
      {props.children}
    </button>
  );
};
