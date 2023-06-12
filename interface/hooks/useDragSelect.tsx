import DragSelect from 'dragselect';
import React, { createContext, useContext, useEffect, useState } from 'react';

type ProviderProps = {
	children: React.ReactNode;
	settings?: ConstructorParameters<typeof DragSelect>[0];
};

const Context = createContext<DragSelect | undefined>(undefined);

function DragSelectProvider({ children, settings = {} }: ProviderProps) {
	const [ds, setDS] = useState<DragSelect>();

	useEffect(() => {
		setDS((prevState) => {
			if (prevState) return prevState;
			return new DragSelect({});
		});
		return () => {
			if (ds) {
				console.log('stop');
				ds.stop();
				setDS(undefined);
			}
		};
	}, [ds]);

	useEffect(() => {
		ds?.setSettings(settings);
	}, [ds, settings]);

	return <Context.Provider value={ds}>{children}</Context.Provider>;
}

function useDragSelect() {
	return useContext(Context);
}

export { DragSelectProvider, useDragSelect };
