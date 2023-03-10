import { useState } from 'react';
import { CRDTOperation, useBridgeSubscription, useLibraryContext } from '~/../packages/client/src';

export default () => {
	const [messages, setMessages] = useState<CRDTOperation[]>([]);

	const { library } = useLibraryContext();

	useBridgeSubscription(
		[
			'sync.messages',
			{
				arg: null,
				library_id: library.uuid
			}
		],
		{
			onData: (msg) => (console.log(msg), setMessages((msgs) => [msg, ...msgs]))
		}
	);

	return (
		<div>
			{messages.map((msg) => (
				<code>{JSON.stringify(msg, null, 4)}</code>
			))}
		</div>
	);
};
