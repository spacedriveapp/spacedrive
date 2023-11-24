import { createMemoryHistory } from '@remix-run/router';
import { QueryClientProvider } from '@tanstack/react-query';
import { listen } from '@tauri-apps/api/event';
import { appWindow } from '@tauri-apps/api/window';
import { useEffect, useMemo, useRef, useState } from 'react';
import { RspcProvider } from '@sd/client';
import {
	ErrorPage,
	KeybindEvent,
	PlatformProvider,
	routes,
	SpacedriveInterface,
	TabsContext
} from '@sd/interface';
import { RouteTitleContext } from '@sd/interface/hooks/useRouteTitle';
import { getSpacedropState } from '@sd/interface/hooks/useSpacedropState';

import '@sd/ui/style/style.scss';

import * as commands from './commands';
import { platform } from './platform';
import { queryClient } from './query';
import { createMemoryRouterWithHistory } from './router';

// TODO: Bring this back once upstream is fixed up.
// const client = hooks.createClient({
// 	links: [
// 		loggerLink({
// 			enabled: () => getDebugState().rspcLogger
// 		}),
// 		tauriLink()
// 	]
// });

const startupError = (window as any).__SD_ERROR__ as string | undefined;

export default function App() {
	useEffect(() => {
		// This tells Tauri to show the current window because it's finished loading
		commands.appReady();
	}, []);

	useEffect(() => {
		const keybindListener = listen('keybind', (input) => {
			document.dispatchEvent(new KeybindEvent(input.payload as string));
		});

		const dropEventListener = appWindow.onFileDropEvent((event) => {
			if (event.payload.type === 'drop') {
				getSpacedropState().droppedFiles = event.payload.paths;
			}
		});

		return () => {
			keybindListener.then((unlisten) => unlisten());
			dropEventListener.then((unlisten) => unlisten());
		};
	}, []);

	return (
		<RspcProvider queryClient={queryClient}>
			<PlatformProvider platform={platform}>
				<QueryClientProvider client={queryClient}>
					{startupError ? (
						<ErrorPage
							message={startupError}
							submessage="Error occurred starting up the Spacedrive core"
						/>
					) : (
						<AppInner />
					)}
				</QueryClientProvider>
			</PlatformProvider>
		</RspcProvider>
	);
}

// we have a minimum delay between creating new tabs as react router can't handle creating tabs super fast
const TAB_CREATE_DELAY = 150;

function AppInner() {
	const [tabs, setTabs] = useState(() => [createTab()]);
	const [tabIndex, setTabIndex] = useState(0);

	function createTab() {
		const history = createMemoryHistory();
		const router = createMemoryRouterWithHistory({ routes, history });

		const dispose = router.subscribe((event) => {
			setTabs((routers) => {
				const index = routers.findIndex((r) => r.router === router);
				if (index === -1) return routers;

				const routerAtIndex = routers[index]!;

				routers[index] = {
					...routerAtIndex,
					currentIndex: history.index,
					maxIndex:
						event.historyAction === 'PUSH'
							? history.index
							: Math.max(routerAtIndex.maxIndex, history.index)
				};

				return [...routers];
			});
		});

		return {
			router,
			history,
			dispose,
			currentIndex: 0,
			maxIndex: 0,
			title: 'New Tab'
		};
	}

	const tab = tabs[tabIndex]!;

	const createTabPromise = useRef(Promise.resolve());

	return (
		<RouteTitleContext.Provider
			value={useMemo(
				() => ({
					setTitle(title) {
						setTabs((oldTabs) => {
							const tabs = [...oldTabs];
							const tab = tabs[tabIndex];
							if (!tab) return tabs;

							tabs[tabIndex] = { ...tab, title };

							return tabs;
						});
					}
				}),
				[tabIndex]
			)}
		>
			<TabsContext.Provider
				value={{
					tabIndex,
					setTabIndex,
					tabs: tabs.map(({ router, title }) => ({ router, title })),
					createTab() {
						createTabPromise.current = createTabPromise.current.then(
							() =>
								new Promise((res) => {
									setTabs((tabs) => {
										const newTabs = [...tabs, createTab()];

										setTabIndex(newTabs.length - 1);

										return newTabs;
									});

									setTimeout(res, TAB_CREATE_DELAY);
								})
						);
					},
					removeTab(index: number) {
						setTabs((tabs) => {
							const tab = tabs[index];
							if (!tab) return tabs;

							tab.dispose();

							tabs.splice(index, 1);

							setTabIndex(tabs.length - 1);

							return [...tabs];
						});
					}
				}}
			>
				<SpacedriveInterface
					routing={{
						router: tab.router,
						routerKey: tabIndex,
						currentIndex: tab.currentIndex,
						maxIndex: tab.maxIndex
					}}
				/>
			</TabsContext.Provider>
		</RouteTitleContext.Provider>
	);
}
