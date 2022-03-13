import type { ClientQuery } from "./ClientQuery";
import type { CoreResource } from "./CoreResource";

export type CoreEvent = { key: "InvalidateQuery", payload: ClientQuery } | { key: "InvalidateResource", payload: CoreResource } | { key: "Log", payload: { message: string, } } | { key: "DatabaseDisconnected", payload: { reason: string | null, } };