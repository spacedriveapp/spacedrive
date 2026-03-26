import {useState} from 'react';

import {ToolCall as SpaceUIToolCall, pairTranscriptSteps} from '@spaceui/ai';
import type {ToolCallPair} from '@spaceui/ai';
export {pairTranscriptSteps};
export type {ToolCallPair};

export function ToolCall({pair}: {pair: ToolCallPair}) {
	const [expanded, setExpanded] = useState(false);

	return (
		<SpaceUIToolCall
			toolCall={pair}
			expanded={expanded}
			onToggle={() => setExpanded(!expanded)}
		/>
	);
}
