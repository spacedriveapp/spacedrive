
export type ClientEvent = { type: "ResourceChange", data: { key: string, id: string, } } | { type: "DatabaseDisconnected", data: { reason: string | null, } };