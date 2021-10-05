import React from 'react';
import clsx from 'clsx';
import { DefaultProps } from './types';
import { Label } from './Input';

interface InputContainerProps extends DefaultProps {
  title: string;
  description?: string;
  children: React.ReactNode;
}

export const InputContainer: React.FC<InputContainerProps> = (props) => {
  return (
    <div className={clsx('', props.className)} {...props}>
      <h3 className="text-gray-700 dark:text-gray-400 font-medium mb-1">{props.title}</h3>
      {!!props.description && <p>{props.description}</p>}
      {props.children}
    </div>
  );
};
