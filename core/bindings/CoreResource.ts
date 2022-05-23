import type { File } from './File';
import type { JobReport } from './JobReport';
import type { LocationResource } from './LocationResource';

export type CoreResource =
	| 'Client'
	| 'Library'
	| { Location: LocationResource }
	| { File: File }
	| { Job: JobReport }
	| 'Tag';
