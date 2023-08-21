import { PropsWithChildren, createContext, useState } from 'react';
import { Notification } from '../core';
import { useBridgeSubscription } from '../rspc';

type Context = {
	notifications: Set<Notification>;
};

const Context = createContext<Context>(null as any);

export function NotificationContextProvider({ children }: PropsWithChildren) {
	const [[notifications], setNotifications] = useState([new Set<Notification>()]);

	useBridgeSubscription(['notifications.listen'], {
		onData(data) {
			setNotifications([notifications.add(data)]);
		}
	});

	return (
		<Context.Provider
			value={{
				notifications
			}}
		>
			{children}
		</Context.Provider>
	);
}
