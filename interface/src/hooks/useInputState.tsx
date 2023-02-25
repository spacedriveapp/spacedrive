import { useState } from 'react';

export function useInputState<T = any>(initialValue: T) {
	const [value, setValue] = useState<T>(initialValue);
	return {
		onChange: (event: React.ChangeEvent<HTMLInputElement>) =>
			setValue(event.target.value as unknown as T),
		value
	};
}
