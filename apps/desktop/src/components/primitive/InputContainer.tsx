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
    <div className="flex flex-row max-w-4xl">
      <div className={clsx('flex flex-col w-full pb-6', props.className)} {...props}>
        <h3 className="text-gray-700 dark:text-gray-100 font-medium mb-1">{props.title}</h3>
        {!!props.description && (
          <p className="text-gray-400 text-sm max-w-md mb-2">{props.description}</p>
        )}
        {!props.mini && props.children}
      </div>
      {props.mini && props.children}
    </div>
  );
};
