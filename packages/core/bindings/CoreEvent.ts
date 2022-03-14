import type { ClientQuery } from "./ClientQuery";
import type { CoreResource } from "./CoreResource";

export type CoreEvent = { key: "InvalidateQuery", data: ClientQuery } | { key: "InvalidateResource", data: CoreResource } | { key: "Log", data: { message: string, } } | { key: "DatabaseDisconnected", data: { reason: string | null, } };