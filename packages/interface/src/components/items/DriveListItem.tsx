import clsx from 'clsx';
import React from 'react';
import { DefaultProps } from '../primitive/types';

export interface DriveListItemProps extends DefaultProps {
  name: string;
}

export const DriveListItem: React.FC<DriveListItemProps> = (props) => {
  return (
    <div
      className={clsx(
        'rounded px-1.5 py-1 text-xs font-medium inline-block cursor-default',
        props.className
      )}
    ></div>
  );
};
