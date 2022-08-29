import { BottomSheetModal } from '@gorhom/bottom-sheet';
import React from 'react';
import create from 'zustand';

import { FilePath } from '../types/bindings';

interface FileModalState {
	fileRef: React.RefObject<BottomSheetModal>;
	data: FilePath | null;
	setData: (data: FilePath) => void;
	clearData: () => void;
}

export const useFileModalStore = create<FileModalState>((set) => ({
	fileRef: React.createRef<BottomSheetModal>(),
	data: null,
	setData: (data: FilePath) => set((_) => ({ data })),
	clearData: () => set((_) => ({ data: null }))
}));
