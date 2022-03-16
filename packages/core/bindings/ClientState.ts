import type { LibraryState } from "./LibraryState";

export interface ClientState { client_uuid: string, client_name: string, data_path: string, tcp_port: number, libraries: Array<LibraryState>, current_library_uuid: string, }