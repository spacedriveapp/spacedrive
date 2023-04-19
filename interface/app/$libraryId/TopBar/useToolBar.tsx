import { useContext } from 'react';
import { useEffect } from 'react';
import { ToolOption } from './ToolBarProvider';
import { ToolBarContext } from './ToolBarProvider';

export const useToolBar = (arg: { options: ToolOption[][] }) => {
	const { setToolBar } = useContext(ToolBarContext);

	useEffect(() => {
		setToolBar(arg);
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, []);
};
