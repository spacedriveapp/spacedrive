
export type ClientQuery = { key: "sys_get_volumes" } | { key: "sys_get_locations", params: { id: string, } } | { key: "lib_explore_path", params: { path: string, limit: number, } };