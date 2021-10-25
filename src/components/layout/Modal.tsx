import React from 'react';

export interface ModalProps {}

export const Modal = (props: ModalProps) => {
  return (
    <div className="w-screen h-screen p-5 absolute t-0 bg-black bg-opacity-30">
      <div className="w-full h-full bg-white rounded-lg shadow-xl"></div>
    </div>
  );
};
