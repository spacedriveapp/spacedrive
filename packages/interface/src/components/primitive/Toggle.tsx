import React from 'react';
import { Switch } from '@headlessui/react';
import clsx from 'clsx';

export interface ToggleProps {
  value: boolean;
  onChange?: (newValue: boolean) => void;
  size?: 'sm' | 'md';
}

export const Toggle: React.FC<ToggleProps> = (props) => {
  const { value: isEnabled = false, onChange = (val) => null, size = 'sm' } = props;

  return (
    <Switch
      checked={isEnabled}
      onChange={onChange}
      className={clsx(
        'transition relative flex-shrink-0 inline-flex items-center h-6 w-11 rounded-full bg-gray-200 dark:bg-gray-550',
        {
          'bg-primary-500 dark:bg-primary-500': isEnabled,
          'h-6 w-11': size === 'sm',
          'h-8 w-[55px]': size === 'md'
        }
      )}
    >
      <span
        className={clsx(
          'transition inline-block w-4 h-4 transform bg-white rounded-full',
          isEnabled ? 'translate-x-6' : 'translate-x-1',
          {
            'w-4 h-4': size === 'sm',
            'h-6 w-6': size === 'md',
            'translate-x-7': size === 'md' && isEnabled
          }
        )}
      />
    </Switch>
  );
};
