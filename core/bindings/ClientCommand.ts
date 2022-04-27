export type ClientCommand =
  | { key: 'FileRead'; params: { id: number } }
  | { key: 'FileDelete'; params: { id: number } }
  | { key: 'LibDelete'; params: { id: number } }
  | { key: 'TagCreate'; params: { name: string; color: string } }
  | { key: 'TagUpdate'; params: { name: string; color: string } }
  | { key: 'TagAssign'; params: { file_id: number; tag_id: number } }
  | { key: 'TagDelete'; params: { id: number } }
  | { key: 'LocCreate'; params: { path: string } }
  | { key: 'LocUpdate'; params: { id: number; name: string | null } }
  | { key: 'LocDelete'; params: { id: number } }
  | { key: 'SysVolumeUnmount'; params: { id: number } }
  | { key: 'GenerateThumbsForLocation'; params: { id: number; path: string } }
  | { key: 'PurgeDatabase' }
  | { key: 'IdentifyUniqueFiles' };
