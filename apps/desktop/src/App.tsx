import { createMemoryHistory } from '@remix-run/router';
import { QueryClientProvider } from '@tanstack/react-query';
import { listen } from '@tauri-apps/api/event';
import { PropsWithChildren, startTransition, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import {
	getItemFilePath,
	libraryClient,
	RspcProvider,
	useBridgeMutation,
	useSelector
} from '@sd/client';
import {
	createRoutes,
	DeeplinkEvent,
	ErrorPage,
	FileDropEvent,
	KeybindEvent,
	PlatformProvider,
	SpacedriveInterfaceRoot,
	SpacedriveRouterProvider,
	TabsContext
} from '@sd/interface';
import { RouteTitleContext } from '@sd/interface/hooks/useRouteTitle';

import '@sd/ui/style';

import { Channel, invoke } from '@tauri-apps/api/core';
import SuperTokens from 'supertokens-web-js';
import EmailPassword from 'supertokens-web-js/recipe/emailpassword';
import Passwordless from 'supertokens-web-js/recipe/passwordless';
import Session from 'supertokens-web-js/recipe/session';
import ThirdParty from 'supertokens-web-js/recipe/thirdparty';
import { explorerStore } from '@sd/interface/app/$libraryId/Explorer/store';
// TODO: Bring this back once upstream is fixed up.
// const client = hooks.createClient({
// 	links: [
// 		loggerLink({
// 			enabled: () => getDebugState().rspcLogger
// 		}),
// 		tauriLink()
// 	]
// });
import getCookieHandler from '@sd/interface/app/$libraryId/settings/client/account/handlers/cookieHandler';
import getWindowHandler from '@sd/interface/app/$libraryId/settings/client/account/handlers/windowHandler';
import { useLocale } from '@sd/interface/hooks';
import { AUTH_SERVER_URL, getTokens } from '@sd/interface/util';

import { Transparent } from '../../../packages/assets/images';
import { commands } from './commands';
import { platform } from './platform';
import { queryClient } from './query';
import { createMemoryRouterWithHistory } from './router';
import { createUpdater } from './updater';

declare global {
	interface Window {
		enableCORSFetch: (enable: boolean) => void;
		useDragAndDrop: () => void;
	}
}

// Disabling until sync is ready.
// SuperTokens.init({
// 	appInfo: {
// 		apiDomain: AUTH_SERVER_URL,
// 		apiBasePath: '/api/auth',
// 		appName: 'Spacedrive Auth Service'
// 	},
// 	cookieHandler: getCookieHandler,
// 	windowHandler: getWindowHandler,
// 	recipeList: [
// 		Session.init({ tokenTransferMethod: 'header' }),
// 		EmailPassword.init(),
// 		ThirdParty.init(),
// 		Passwordless.init()
// 	]
// });

const startupError = (window as any).__SD_ERROR__ as string | undefined;

function useDragAndDrop() {
	const dragState = useSelector(explorerStore, (s) => s.drag);

	useEffect(() => {
		console.log('Drag effect triggered:', {
			dragStateType: dragState?.type,
			itemCount: dragState?.type === 'dragging' ? dragState?.items?.length : undefined
		});

		(async () => {
			if (['linux', 'browser'].includes(await platform.getOs())) {
				console.log('Skipping drag operation on Linux or Browser');
				return;
			}
			if (dragState?.type === 'dragging' && dragState.items.length > 0) {
				console.log('Starting drag operation with items:', dragState.items);

				const items = await Promise.all(
					dragState.items.map(async (item) => {
						const data = getItemFilePath(item);
						if (!data) {
							console.log('No file path data for item:', item);
							return;
						}

						const file_path =
							'path' in data
								? data.path
								: await libraryClient.query(['files.getPath', data.id]);

						console.log('Resolved file path:', file_path);
						return {
							type: 'explorer-item',
							file_path: file_path
						};
					})
				);

				const validFiles = items.filter(Boolean).map((item) => item?.file_path);
				console.log('Invoking start_drag with files:', validFiles);

				try {
					const channel = new Channel<{
						result: 'Dropped' | 'Cancelled';
						cursorPos: { x: number; y: number };
					}>();

					channel.onmessage = (payload) => {
						console.log('Drag completed:', {
							result: payload.result,
							position: payload.cursorPos,
							timestamp: new Date().toISOString()
						});

						if (payload.result === 'Dropped') {
							console.log('Drop location:', {
								x: payload.cursorPos.x,
								y: payload.cursorPos.y,
								screen: window.screen
							});
							// Refetch explorer files after successful drop
							queryClient.invalidateQueries({ queryKey: ['search.paths'] });
						}

						explorerStore.drag = null;
					};

					const image = !Transparent.includes('/@fs/')
						? Transparent
						: Transparent.replace('/@fs', '');

					await invoke('start_drag', {
						files: validFiles,
						image: image,
						onEvent: channel
					});
					console.log('start_drag invoked successfully');
				} catch (error) {
					console.error('Failed to start drag:', error);
					explorerStore.drag = null;
				}
			} else {
				console.log('Drag operation cancelled');
				await invoke('stop_drag');
			}
		})();
	}, [dragState]);
}

export default function App() {
	useEffect(() => {
		// This tells Tauri to show the current window because it's finished loading
		commands.appReady();
		window.enableCORSFetch(true);
		window.useDragAndDrop = useDragAndDrop;
		// .then(() => {
		// 	if (import.meta.env.PROD) window.fetch = fetch;
		// });
	}, []);

	useEffect(() => {
		const keybindListener = listen('keybind', (input) => {
			document.dispatchEvent(new KeybindEvent(input.payload as string));
		});
		const deeplinkListener = listen('deeplink', async (data) => {
			const payload = (data.payload as any).data as string;
			if (!payload) return;
			const json = JSON.parse(payload)[0];
			if (!json) return;
			//json output: "spacedrive://-/URL"
			if (typeof json !== 'string') return;
			if (!json.startsWith('spacedrive://-')) return;
			const url = (json as string).split('://-/')[1];
			if (!url) return;
			document.dispatchEvent(new DeeplinkEvent(url));
		});
		const fileDropListener = listen('tauri://drag-drop', async (data) => {
			document.dispatchEvent(new FileDropEvent((data.payload as { paths: string[] }).paths));
		});

		return () => {
			keybindListener.then((unlisten) => unlisten());
			deeplinkListener.then((unlisten) => unlisten());
			fileDropListener.then((unlisten) => unlisten());
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

type RedirectPath = { pathname: string; search: string | undefined };

function AppInner() {
	const [tabs, setTabs] = useState(() => [createTab()]);
	const [selectedTabIndex, setSelectedTabIndex] = useState(0);
	const cloudBootstrap = useBridgeMutation('cloud.bootstrap');

	useEffect(() => {
		(async () => {
			const tokens = await getTokens();
			// If the access token and/or refresh token are missing, we need to skip the cloud bootstrap
			if (tokens.accessToken.length === 0 || tokens.refreshToken.length === 0) return;
			cloudBootstrap.mutate([tokens.accessToken, tokens.refreshToken]);
		})();
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, []);

	const selectedTab = tabs[selectedTabIndex]!;

	function createTab(redirect?: RedirectPath) {
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

	const SizeDisplay = () => {
		const [size, setSize] = useState({
			width: window.innerWidth,
			height: window.innerHeight
		});

		useEffect(() => {
			const handleResize = () => {
				setSize({
					width: window.innerWidth,
					height: window.innerHeight
				});
			};

			window.addEventListener('resize', handleResize);
			return () => window.removeEventListener('resize', handleResize);
		}, []);

		return (
			<div
				style={{
					position: 'fixed',
					bottom: 10,
					right: 10,
					background: 'rgba(0,0,0,0.7)',
					color: 'white',
					padding: '5px 10px',
					borderRadius: '5px',
					fontSize: '12px',
					zIndex: 9999
				}}
			>
				{size.width} x {size.height}
			</div>
		);
	};

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
					createTab(redirect?: RedirectPath) {
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
					duplicateTab() {
						createTabPromise.current = createTabPromise.current.then(
							() =>
								new Promise((res) => {
									startTransition(() => {
										setTabs((tabs) => {
											const { pathname, search } =
												selectedTab.router.state.location;
											const newTab = createTab({ pathname, search });
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
						{/* <SizeDisplay /> */}
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
