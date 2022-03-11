// import create from 'zustand';
// import immer, { produce } from 'immer';

// export interface Resource {
//   id: string;
// }

// interface ResourceStore<R = Resource> {
//   locations: Record<string, R>;
//   setResources: (resource: R[]) => void;
// }

// export const useResourceStore = create<ResourceStore>((set, get) => ({
//   locations: {},
//   setResources: (locations) =>
//     set((state) =>
//       produce(state, (draft) => {
//         for (let location of locations) {
//           draft.locations[location.path] = location;
//         }
//       })
//     )
// }));

// export const useResources = () => {
//   return useResourceStore((store) => Object.values(store.locations));
// };

// export function createResource<R extends Resource>() {

// }
