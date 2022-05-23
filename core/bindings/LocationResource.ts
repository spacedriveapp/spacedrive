export interface LocationResource {
	id: number;
	name: string | null;
	path: string | null;
	total_capacity: number | null;
	available_capacity: number | null;
	is_removable: boolean | null;
	is_online: boolean;
	date_created: string;
}
