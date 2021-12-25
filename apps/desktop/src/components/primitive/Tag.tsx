import clsx from 'clsx';
import React from 'react';
import { DefaultProps } from './types';

export interface TagProps extends DefaultProps {}

export const Tag: React.FC<TagProps> = (props) => {
  return <div className={clsx('rounded px-2 py-1', props.className)}>{props.children}</div>;
};
