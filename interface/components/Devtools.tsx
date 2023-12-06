import { defaultContext } from '@tanstack/react-query';
import { ReactQueryDevtools } from '@tanstack/react-query-devtools';
import { useDebugState } from '@sd/client';

export const Devtools = () => {
	const debugState = useDebugState();

	return (
		<>
			{debugState.reactQueryDevtools !== 'disabled' ? (
				<ReactQueryDevtools
					position="bottom-right"
					// The `context={defaultContext}` part is required for this to work on Windows.
					// Why, idk, don't question it
					context={defaultContext}
					toggleButtonProps={{
						tabIndex: -1,
						className: debugState.reactQueryDevtools === 'invisible' ? 'opacity-0' : ''
					}}
				/>
			) : null}
		</>
	);
};
