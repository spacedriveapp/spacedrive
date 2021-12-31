import React from 'react';
import { DefaultProps } from '../primitive/types';

interface Props extends DefaultProps {
  iconName: string;
  fileName: string;
  format: string;
}

export default function FileItem(props: Props) {
  return (
    <div className="inline-block text-center w-[100px]  h-[100px] mb-6">
      {/* <div className="w-[65px] border border-gray-600 m-auto rounded-md h-[80px] bg-gray-650 relative shadow-md "> */}
      <div className="w-[65px] m-auto rounded-md h-[80px] relative  ">
        <svg
          className="absolute top-0 left-0 shadow-md fill-gray-650"
          width="65"
          height="81"
          viewBox="0 0 65 81"
        >
          <path d="M0 8C0 3.58172 3.58172 0 8 0H39.6863C41.808 0 43.8429 0.842855 45.3431 2.34315L53.5 10.5L62.6569 19.6569C64.1571 21.1571 65 23.192 65 25.3137V73C65 77.4183 61.4183 81 57 81H8C3.58172 81 0 77.4183 0 73V8Z" />
        </svg>
        <svg
          width="23"
          height="23"
          className="absolute right-0 z-10 shadow-md fill-gray-550"
          viewBox="0 0 41 41"
        >
          <path d="M41.4116 40.5577H11.234C5.02962 40.5577 0 35.5281 0 29.3238V0L41.4116 40.5577Z" />
        </svg>
        <div className="absolute flex flex-col items-center justify-center w-full h-full">
          <img
            className="mt-2 margin-auto"
            width={40}
            height={40}
            src={`assets/icons/${props.iconName}.svg`}
          />
          <span className="font-bold text-center uppercase cursor-default text-gray-550">
            {props.format}
          </span>
        </div>
      </div>
      <p className="mt-1 text-sm font-medium text-gray-300 cursor-default">{props.fileName}</p>
    </div>
  );
}
