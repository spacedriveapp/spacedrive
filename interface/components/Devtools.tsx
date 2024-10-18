import { ReactQueryDevtools } from '@tanstack/react-query-devtools';
import { useDebugState } from '@sd/client';

export const Devtools = () => {
	const debugState = useDebugState();

	return (
		<>
			{debugState.reactQueryDevtools && (
				<ReactQueryDevtools buttonPosition="bottom-right" position="bottom" />
			)}
		</>
	);
};
