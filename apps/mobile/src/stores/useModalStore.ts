import { BottomSheetModal } from '@gorhom/bottom-sheet';
import React from 'react';
import create from 'zustand';

interface FileModalState {
	fileRef: React.RefObject<BottomSheetModal>;
	data: any;
	setData: (data: any) => void;
	clearData: () => void;
}

export const useFileModalStore = create<FileModalState>((set) => ({
	fileRef: React.createRef<BottomSheetModal>(),
	data: null,
	setData: (data: any) => set((_) => ({ data })),
	clearData: () => set((_) => ({ data: null }))
}));
