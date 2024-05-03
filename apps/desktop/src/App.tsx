import { createMemoryHistory } from '@remix-run/router';
import { QueryClientProvider } from '@tanstack/react-query';
import { listen } from '@tauri-apps/api/event';
import { PropsWithChildren, startTransition, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { RspcProvider } from '@sd/client';
import {
	createRoutes,
	ErrorPage,
	KeybindEvent,
	PlatformProvider,
	SpacedriveInterfaceRoot,
	SpacedriveRouterProvider,
	TabsContext
} from '@sd/interface';
import { RouteTitleContext } from '@sd/interface/hooks/useRouteTitle';

import '@sd/ui/style/style.scss';

import { useLocale } from '@sd/interface/hooks';

import { commands } from './commands';
import { platform } from './platform';
import { queryClient } from './query';
import { createMemoryRouterWithHistory } from './router';
import { createUpdater } from './updater';

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

		return () => {
			keybindListener.then((unlisten) => unlisten());
		};
	}, []);

	return (
		<RspcProvider queryClient={queryClient}>
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
		</RspcProvider>
	);
}

// we have a minimum delay between creating new tabs as react router can't handle creating tabs super fast
const TAB_CREATE_DELAY = 150;

const routes = createRoutes(platform);

type redirect = { pathname: string; search: string | undefined };

function AppInner() {
	const [tabs, setTabs] = useState(() => [createTab()]);
	const [selectedTabIndex, setSelectedTabIndex] = useState(0);

	const selectedTab = tabs[selectedTabIndex]!;

	function createTab(redirect?: redirect) {
		const history = createMemoryHistory();
		const router = createMemoryRouterWithHistory({ routes, history });

		const id = Math.random().toString();

		// for "Open in new tab"
		if (redirect) {
			router.navigate({
				pathname: redirect.pathname,
				search: redirect.search
			});
		}

		const dispose = router.subscribe((event) => {
			// we don't care about non-idle events as those are artifacts of form mutations + suspense
			if (event.navigation.state !== 'idle') return;

			setTabs((routers) => {
				const index = routers.findIndex((r) => r.id === id);
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
			id,
			router,
			history,
			dispose,
			element: document.createElement('div'),
			currentIndex: 0,
			maxIndex: 0,
			title: 'New Tab'
		};
	}

	const createTabPromise = useRef(Promise.resolve());

	const ref = useRef<HTMLDivElement>(null);

	useEffect(() => {
		const div = ref.current;
		if (!div) return;

		div.appendChild(selectedTab.element);

		return () => {
			while (div.firstChild) {
				div.removeChild(div.firstChild);
			}
		};
	}, [selectedTab.element]);

	return (
		<RouteTitleContext.Provider
			value={useMemo(
				() => ({
					setTitle(id, title) {
						setTabs((tabs) => {
							const tabIndex = tabs.findIndex((t) => t.id === id);
							if (tabIndex === -1) return tabs;

							tabs[tabIndex] = { ...tabs[tabIndex]!, title };

							return [...tabs];
						});
					}
				}),
				[]
			)}
		>
			<TabsContext.Provider
				value={{
					tabIndex: selectedTabIndex,
					setTabIndex: setSelectedTabIndex,
					tabs: tabs.map(({ router, title }) => ({ router, title })),
					createTab(redirect?: redirect) {
						createTabPromise.current = createTabPromise.current.then(
							() =>
								new Promise((res) => {
									startTransition(() => {
										setTabs((tabs) => {
											const newTab = createTab(redirect);
											const newTabs = [...tabs, newTab];

											setSelectedTabIndex(newTabs.length - 1);

											return newTabs;
										});
									});

									setTimeout(res, TAB_CREATE_DELAY);
								})
						);
					},
					removeTab(index: number) {
						startTransition(() => {
							setTabs((tabs) => {
								const tab = tabs[index];
								if (!tab) return tabs;

								tab.dispose();

								tabs.splice(index, 1);

								setSelectedTabIndex(Math.min(selectedTabIndex, tabs.length - 1));

								return [...tabs];
							});
						});
					}
				}}
			>
				<PlatformUpdaterProvider>
					<SpacedriveInterfaceRoot>
						{tabs.map((tab, index) =>
							createPortal(
								<SpacedriveRouterProvider
									key={tab.id}
									routing={{
										routes,
										visible: selectedTabIndex === tabs.indexOf(tab),
										router: tab.router,
										currentIndex: tab.currentIndex,
										tabId: tab.id,
										maxIndex: tab.maxIndex
									}}
								/>,
								tab.element
							)
						)}
						<div ref={ref} />
					</SpacedriveInterfaceRoot>
				</PlatformUpdaterProvider>
			</TabsContext.Provider>
		</RouteTitleContext.Provider>
	);
}

function PlatformUpdaterProvider(props: PropsWithChildren) {
	const { t } = useLocale();

	return (
		<PlatformProvider
			platform={useMemo(
				() => ({
					...platform,
					updater: window.__SD_UPDATER__ ? createUpdater(t) : undefined
				}),
				[t]
			)}
		>
			{props.children}
		</PlatformProvider>
	);
}
