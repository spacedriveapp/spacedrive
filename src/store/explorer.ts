import create from 'zustand';
import { IDirectory, IFile } from '../types';
import produce from 'immer';

interface ExplorerStore {
  dirs: Record<string, IDirectory>;
  activeDirHash: string;
  history: string[];
  selected: null | { index: number; file: IFile };
  collectDir: (dirHash: IFile, files: IFile[]) => void;
  currentDir?: () => IFile[];
  setSelected: (index: number | null, file?: IFile) => void;
  goBack: () => void;
}

export const useExplorerStore = create<ExplorerStore>((set, get) => ({
  dirs: {},
  activeDirHash: '',
  history: [],
  selected: null,
  collectDir: (directory, files) => {
    set((state) =>
      produce(state, (draft) => {
        draft.history.push(directory.meta_checksum);
        draft.activeDirHash = directory.meta_checksum;
        draft.dirs[directory.meta_checksum] = {
          children: files,
          children_count: files.length,
          ...directory
        };
      })
    );
  },
  goBack: () => {
    if (get().history.length > 1) {
      set((state) =>
        produce(state, (draft) => {
          draft.history.pop();
          draft.activeDirHash = draft.history[draft.history.length - 1];
        })
      );
    }
  },
  setSelected: (index?: number | null, file?: IFile) =>
    set((state) =>
      produce(state, (draft) => {
        draft.selected = !index || !file ? null : { index, file };
      })
    )
}));
