import clsx from 'clsx';
import React from 'react';
import { DefaultProps } from '../primative/types';

export interface TrafficLightsProps extends DefaultProps {
  onClose?: () => void;
  onMinimize?: () => void;
  onFullscreen?: () => void;
}

export const TrafficLights: React.FC<TrafficLightsProps> = (props) => {
  return (
    <div className={clsx('flex flex-row space-x-1.5', props.className)}>
      <Light mode="close" action={props.onClose} />
      <Light mode="minimize" action={props.onMinimize} />
      <Light mode="fullscreen" action={props.onFullscreen} />
    </div>
  );
};

interface LightProps {
  mode: 'close' | 'minimize' | 'fullscreen';
  action?: () => void;
}

const Light: React.FC<LightProps> = (props) => {
  return (
    <div
      className={clsx('w-3.5 h-3.5 rounded-full', {
        'bg-red-400': props.mode == 'close',
        'bg-green-400': props.mode == 'fullscreen',
        'bg-yellow-400': props.mode == 'minimize'
      })}
    ></div>
  );
};
