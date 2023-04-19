import { useEffect } from 'react';
import { ToolOption, useToolBarContext } from './ToolBarProvider';

export const useToolBar = (arg: { options: ToolOption[][] }) => {
	const { setToolBar } = useToolBarContext();

	useEffect(() => {
		setToolBar(arg);
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, []);
};
