import create from 'zustand';
import { IDirectory, IFile } from '../types';
import produce from 'immer';

interface ExplorerStore {
  // storage
  files: Record<number, IFile>;
  // indexes
  dirs: Record<number, number[]>;
  // ingest
  ingestDir: (dir: IFile, children: IFile[]) => void;
  // selection
  selectedFile: number | null;
  selectedFiles: number[];
  selectedFilesHistory: string[][];
  selectFile: (dirId: number, fileId: number, type?: 'below' | 'above') => void;
  clearSelectedFiles: () => void;
  // selectAnotherFile?: (fileId: number) => void;
  // selectFilesBetween?: (firstFileId: number, secondFileId: number) => void;
  // explorer state
  currentDir: number | null;
  dirHistory: number[];
  goBack?: () => void;
  goForward?: () => void;

  tempInjectThumb: (fileId: number, b64: string) => void;
}

export const useExplorerStore = create<ExplorerStore>((set, get) => ({
  files: {},
  dirs: {},
  selectedFile: null,
  selectedFiles: [],
  selectedFilesHistory: [],
  currentDir: null,
  dirHistory: [],

  ingestDir: (dir, children) => {
    set((state) =>
      produce(state, (draft) => {
        draft.files[dir.id] = dir;
        // extract directory index of file ids
        draft.dirs[dir.id] = children.map((file) => file.id);
        // save files by id
        for (const index in children) {
          const child = children[index];
          draft.files[child.id] = child;
        }
        // if this dir in the history stack, remove history since then
        const existingDirHistoryIndex = draft.dirHistory.findIndex((i) => i === dir.id);
        if (existingDirHistoryIndex != -1) {
          draft.dirHistory.splice(existingDirHistoryIndex);
        }
        // push onto history stack
        draft.dirHistory.push(dir.id);

        draft.currentDir = dir.id;
      })
    );
  },
  goBack: () => {
    set((state) =>
      produce(state, (draft) => {
        const prevDirId = draft.dirHistory[draft.dirHistory.length - 2];
        if (prevDirId == undefined) return;
        draft.currentDir = prevDirId;
      })
    );
  },
  goForward: () => {
    set((state) =>
      produce(state, (draft) => {
        const nextDirId = draft.dirHistory[draft.dirHistory.length];
        if (nextDirId == undefined) return;
        draft.currentDir = nextDirId;
      })
    );
  },
  selectFile: (dirId, fileId, type) => {
    set((state) =>
      produce(state, (draft) => {
        if (!draft.files[fileId]) return;
        if (!type) {
          draft.selectedFile = fileId;
        }
        // this is the logic for up / down movement on selected file
        const dirIndex = draft.dirs[dirId];
        const maxIndex = dirIndex.length - 1;
        const activeIndex = dirIndex.findIndex((i) => i === fileId);
        switch (type) {
          case 'above':
            if (activeIndex - 1 < 0) draft.selectedFile = dirIndex[maxIndex];
            else draft.selectedFile = dirIndex[activeIndex - 1];
            break;
          case 'below':
            if (activeIndex + 1 > maxIndex) draft.selectedFile = dirIndex[0];
            else draft.selectedFile = dirIndex[activeIndex + 1];
            break;
        }
      })
    );
  },
  clearSelectedFiles: () => {
    set((state) =>
      produce(state, (draft) => {
        draft.selectedFile = null;
      })
    );
  },
  tempInjectThumb: (fileId: number, b64: string) => {
    set((state) =>
      produce(state, (draft) => {
        if (!draft.files[fileId]) return;
        draft.files[fileId].icon_b64 = b64;
      })
    );
  }
}));

export function useSelectedFile(): null | IFile {
  const [file] = useExplorerStore((state) => [state.files[state.selectedFile || -1]]);
  return file;
}

export function useSelectedFileIndex(dirId: number): null | number {
  return useExplorerStore((state) =>
    state.dirs[dirId].findIndex((i) => i === state.files[state.selectedFile || -1]?.id)
  );
}

export function useFile(fileId: number): null | IFile {
  return useExplorerStore((state) => state.files[fileId || -1]);
}

export function useCurrentDir(): IDirectory | null {
  return useExplorerStore((state) => {
    const children = state.dirs[state.currentDir || -1].map((id) => state.files[id]);
    const directory = state.files[state.currentDir || -1];

    return {
      ...directory,
      children,
      children_count: children.length
    };
  });
}
