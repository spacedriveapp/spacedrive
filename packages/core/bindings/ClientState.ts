import type { LibraryState } from "./LibraryState";

export interface ClientState { client_id: string, client_name: string, data_path: string, tcp_port: number, libraries: Array<LibraryState>, current_library_id: string, }