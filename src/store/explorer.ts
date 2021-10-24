import create from 'zustand';
import { IDirectory, IFile } from '../types';
import produce from 'immer';
import { useCallback, useMemo } from 'react';

interface SelectedFile {
  id: number;
  index?: number;
}

interface ExplorerStore {
  // storage
  files: Record<number, IFile>;
  // indexes
  dirs: Record<number, number[]>;
  // ingest
  ingestDir: (dir: IFile, children: IFile[]) => void;
  // selection
  selectedFile: SelectedFile | null;
  selectedFiles: SelectedFile[];
  selectedFilesHistory: SelectedFile[][];
  selectFile: (
    dirId: number,
    fileId: number,
    type?: 'below' | 'above',
    specificIndex?: number
  ) => void;
  clearSelectedFiles: () => void;
  // selectAnotherFile?: (fileId: number) => void;
  // selectFilesBetween?: (firstFileId: number, secondFileId: number) => void;
  // explorer state
  currentDir: number | null;
  dirHistory: number[];
  goBack?: () => void;
  goForward?: () => void;
  nativeIconUpdated: (fileId: number) => void;

  tempWatchDir: string;
  setTempWatchDir: (path: string) => void;
}

export const useExplorerStore = create<ExplorerStore>((set, get) => ({
  files: {},
  dirs: {},
  selectedFile: null,
  selectedFileIndex: null,
  selectedFiles: [],
  selectedFilesHistory: [],
  currentDir: null,
  dirHistory: [],
  tempWatchDir: '/Users/jamie/Downloads',
  setTempWatchDir: (path) =>
    set((state) =>
      produce(state, (draft) => {
        draft.tempWatchDir = path;
      })
    ),

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
  selectFile: (dirId, fileId, type, specificIndex) => {
    set((state) =>
      produce(state, (draft) => {
        if (!draft.files[fileId]) return;
        const dirIndex = get().dirs[dirId];
        const maxIndex = dirIndex.length - 1;
        // discover index within active directory

        const currentIndex =
          state.selectedFile?.index !== undefined
            ? state.selectedFile.index
            : (() => {
                console.log('FINDING INDEX');

                return dirIndex.findIndex((i) => i === fileId);
              })();
        console.log('selecting file', { fileId, dirIndex, maxIndex, currentIndex });
        // if no type just select specified file
        if (!type) {
          draft.selectedFile = { id: fileId, index: specificIndex };
          return;
        }
        switch (type) {
          case 'above':
            if (currentIndex - 1 < 0)
              draft.selectedFile = { id: dirIndex[maxIndex], index: maxIndex };
            else draft.selectedFile = { id: dirIndex[currentIndex - 1], index: currentIndex - 1 };
            break;
          case 'below':
            if (currentIndex + 1 > maxIndex) draft.selectedFile = { id: dirIndex[0], index: 0 };
            else draft.selectedFile = { id: dirIndex[currentIndex + 1], index: currentIndex + 1 };
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
  nativeIconUpdated: (fileId: number) => {
    set((state) =>
      produce(state, (draft) => {
        if (!draft.files[fileId]) return;
        draft.files[fileId].has_native_icon = true;
      })
    );
  }
}));

export function useSelectedFile(): null | IFile {
  const [file] = useExplorerStore((state) => [state.files[state.selectedFile?.id || -1]]);
  return file;
}

export function useSelectedFileIndex(dirId: number): null | number {
  return useExplorerStore((state) => {
    const index = state.selectedFile?.index;
    if (index === undefined) return null;
    return index;
  });
}

export function useFile(fileId: number): null | IFile {
  return useExplorerStore((state) => state.files[fileId || -1]);
}

export function useCurrentDir(): IDirectory | null {
  return useExplorerStore((state) => {
    const children = useMemo(
      () => state.dirs[state.currentDir || -1].map((id) => state.files[id]),
      [state.currentDir]
    );
    const directory = state.files[state.currentDir || -1];

    return {
      ...directory,
      children,
      children_count: children.length
    };
  });
}
