import type { LibraryState } from "./LibraryState";

export interface NodeState { node_pub_id: string, node_id: number, node_name: string, data_path: string, tcp_port: number, libraries: Array<LibraryState>, current_library_uuid: string, }