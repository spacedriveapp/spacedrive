import {
	Dispatch,
	PropsWithChildren,
	SetStateAction,
	createContext,
	useContext,
	useState
} from 'react';

export interface ToolOption {
	icon: JSX.Element;
	onClick?: () => void;
	individual?: boolean;
	toolTipLabel: string;
	topBarActive?: boolean;
	popOverComponent?: JSX.Element;
	showAtResolution: ShowAtResolution;
}

export type ShowAtResolution = 'sm:flex' | 'md:flex' | 'lg:flex' | 'xl:flex' | '2xl:flex';

interface ToolBarContext {
	toolBar: { options: ToolOption[][] };
	setToolBar: Dispatch<SetStateAction<{ options: ToolOption[][] }>>;
}

export const TOP_BAR_ICON_STYLE = 'm-0.5 w-5 h-5 text-ink-dull';

export const ToolBarContext = createContext<ToolBarContext>({
	toolBar: { options: [[]] },
	setToolBar: () => {}
});
export function useTopBar() {
	const ctx = useContext(ToolBarContext);
	if (!ctx) throw new Error('Missing ToolBarContext');
	return ctx;
}

export default ({ children }: PropsWithChildren) => {
	const [toolBar, setToolBar] = useState<{ options: ToolOption[][] }>({
		options: [[]]
	});
	return (
		<ToolBarContext.Provider value={{ toolBar, setToolBar }}>
			{children}
		</ToolBarContext.Provider>
	);
};
