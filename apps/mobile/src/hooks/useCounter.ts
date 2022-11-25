import { useEffect } from 'react';
import { useCountUp } from 'use-count-up';
import { proxy, useSnapshot } from 'valtio';

const counterStore = proxy({
	counterLastValue: new Map<string, number>(),
	setCounterLastValue: (key: string, value: number) => {
		counterStore.counterLastValue.set(key, value);
	}
});

const useCounterState = (key: string) => {
	const { counterLastValue, setCounterLastValue } = useSnapshot(counterStore);

	return {
		lastValue: counterLastValue.get(key),
		setLastValue: setCounterLastValue
	};
};

type UseCounterProps = {
	name: string;
	start?: number;
	end: number;
	/**
	 * Duration of the counter animation in seconds
	 * default: `2s`
	 */
	duration?: number;
	/**
	 * If `true`, counter will only count up/down once per app session.
	 * default: `true`
	 */
	saveState?: boolean;
};

const useCounter = ({ name, start = 0, end, duration = 2, saveState = true }: UseCounterProps) => {
	const { lastValue, setLastValue } = useCounterState(name);

	if (saveState && lastValue) {
		start = lastValue;
	}

	const { value } = useCountUp({
		isCounting: !(start === end),
		start,
		end,
		duration,
		easing: 'easeOutCubic'
	});

	useEffect(() => {
		if (saveState && Number(value) === end) {
			setLastValue(name, end);
		}
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [end, value]);

	if (start === end) return end;

	if (saveState && lastValue && lastValue === end) return end;

	return value;
};

export default useCounter;
