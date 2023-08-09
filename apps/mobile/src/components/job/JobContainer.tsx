import { Icon } from 'phosphor-react-native';
import { ViewStyle } from 'react-native';
import { TextItems } from '@sd/client';

type JobContainerProps = {
	name: string;
	icon?: string | Icon;
	// Array of arrays of TextItems, where each array of TextItems is a truncated line of text.
	textItems?: TextItems;
	isChild?: boolean;
	children: React.ReactNode;
	containerStyle?: ViewStyle;
};

// Job container consolidates the common layout of a job item, used for regular jobs (Job.tsx) and grouped jobs (JobGroup.tsx).
export default function JobContainer(props: JobContainerProps) {
	return <></>;
}
