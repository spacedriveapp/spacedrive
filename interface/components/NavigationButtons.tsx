import { ArrowLeft, ArrowRight } from 'phosphor-react';
import { useNavigate } from 'react-router';
import { Button, Tooltip } from '@sd/ui';
import { useSearchStore } from '~/hooks/useSearchStore';

export default () => {
	const navigate = useNavigate();
	const { isFocused } = useSearchStore();
	const idx = history.state.idx as number;

	return (
		<div className="flex">
			<Tooltip label="Navigate back">
				<Button
					size="icon"
					className="text-[14px] text-ink-dull"
					onClick={() => navigate(-1)}
					disabled={isFocused || idx === 0}
				>
					<ArrowLeft weight="bold" />
				</Button>
			</Tooltip>
			<Tooltip label="Navigate forward">
				<Button
					size="icon"
					className="text-[14px] text-ink-dull"
					onClick={() => navigate(1)}
					disabled={isFocused || idx === history.length - 1}
				>
					<ArrowRight weight="bold" />
				</Button>
			</Tooltip>
		</div>
	);
};
