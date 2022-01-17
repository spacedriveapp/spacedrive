import type { Platform } from "./Platform";

export interface Client { id: number, name: string, platform: Platform, online: boolean, last_seen: string, timezone: string | null, date_created: string, }