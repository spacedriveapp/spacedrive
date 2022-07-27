import { useCountUp } from 'use-count-up';
import create from 'zustand';

const useStatItemStore = create<{
	statItemLastValue: Map<string, number>;
	setStatItemLastValue(key: string, value: number): void;
}>((set) => ({
	statItemLastValue: new Map<string, number>(),
	setStatItemLastValue: (name, lastValue) =>
		set((state) => ({
			...state,
			statItemLastValue: state.statItemLastValue.set(name, lastValue)
		}))
}));

const useStatItemState = (key: string) => {
	const { statItemLastValue, setStatItemLastValue } = useStatItemStore();

	return {
		lastValue: statItemLastValue.get(key),
		setLastValue: setStatItemLastValue
	};
};

type StatCounterProps = {
	name: string;
	start?: number;
	end: number;
};

const useStatCounter = ({ name, start = 0, end }: StatCounterProps) => {
	const { lastValue, setLastValue } = useStatItemState(name);

	if (lastValue) {
		start = lastValue;
	}

	const { value } = useCountUp({
		isCounting: !(start === end),
		start,
		end,
		duration: 2,
		easing: 'easeOutCubic'
	});

	if (start === end) return end;

	if (lastValue && lastValue === end) return end;

	if (value == end) {
		setLastValue(name, end);
	}

	return value;
};

export default useStatCounter;
