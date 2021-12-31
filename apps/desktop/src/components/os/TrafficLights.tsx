import clsx from 'clsx';
import React from 'react';
import { useFocusState } from '../../hooks/useFocusState';
import { DefaultProps } from '../primitive/types';

export interface TrafficLightsProps extends DefaultProps {
  onClose?: () => void;
  onMinimize?: () => void;
  onFullscreen?: () => void;
}

export const TrafficLights: React.FC<TrafficLightsProps> = (props) => {
  const [focused] = useFocusState()
  return (
    <div className={clsx('flex flex-row space-x-2 px-2 group', props.className)}>
      <Light mode="close" action={props.onClose} focused={focused} />
      <Light mode="minimize" action={props.onMinimize} focused={focused} />
      <Light mode="fullscreen" action={props.onFullscreen} focused={focused} />
    </div>
  );
};

interface LightProps {
  mode: 'close' | 'minimize' | 'fullscreen';
  focused: boolean;
  action?: () => void;
}

const Light: React.FC<LightProps> = (props) => {
  return (
    <div
      onClick={props.action}
      className={clsx('w-[13px] h-[13px] rounded-full bg-gray-500', {
        '!bg-red-400': props.mode == 'close' && props.focused,
        '!bg-green-400': props.mode == 'fullscreen' && props.focused,
        '!bg-yellow-400': props.mode == 'minimize' && props.focused
      })}
    ></div>
  );
};
