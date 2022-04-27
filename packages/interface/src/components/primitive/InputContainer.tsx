import React from 'react';
import clsx from 'clsx';
import { DefaultProps } from './types';

interface InputContainerProps extends DefaultProps {
  title: string;
  description?: string;
  children: React.ReactNode;
  mini?: boolean;
}

export const InputContainer: React.FC<InputContainerProps> = (props) => {
  return (
    <div className="flex flex-row">
      <div
        className={clsx('flex flex-col w-full', !props.mini && 'pb-6', props.className)}
        {...props}
      >
        <h3 className="mb-1 font-medium text-gray-700 dark:text-gray-100">{props.title}</h3>
        {!!props.description && <p className="mb-2 text-sm text-gray-400 ">{props.description}</p>}
        {!props.mini && props.children}
      </div>
      {props.mini && props.children}
    </div>
  );
};
