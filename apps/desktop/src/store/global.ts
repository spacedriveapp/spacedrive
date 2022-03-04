import produce from 'immer';
import create from 'zustand';

export interface AppState {
  config: {
    primary_db?: string;
    data_dir?: string;
    file_type_thumb_dir?: string;
  };
}

interface AppStoreState extends AppState {
  update: (newObj: AppState) => void;
}

export const useAppState = create<AppStoreState>((set, get) => ({
  config: {},
  update: (newObj) => {
    set((store) =>
      produce(store, (draft) => {
        Object.keys(newObj).forEach((key) => {
          //@ts-ignore
          draft.config[key as keyof AppState] = newObj[key];
        });
      })
    );
  }
}));
