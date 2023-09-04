import { useMemo } from 'react';
import { stringify } from 'uuid';
import {
	CRDTOperation,
	RelationOperation,
	SharedOperation,
	useLibraryQuery,
	useLibrarySubscription
} from '@sd/client';

type MessageGroup =
	| {
			variant: 'Shared';
			model: string;
			id: string;
			messages: { op: SharedOperation; timestamp: number }[];
	  }
	| {
			variant: 'Relation';
			relation: string;
			item_id: string;
			group_id: string;
			messages: { op: RelationOperation; timestamp: number }[];
	  };

export const Component = () => {
	const messages = useLibraryQuery(['sync.messages']);

	useLibrarySubscription(['sync.newMessage'], {
		onData: () => messages.refetch()
	});

	const groups = useMemo(() => {
		if (messages.data) return calculateGroups(messages.data);
	}, [messages]);

	return (
		<ul className="space-y-4 p-4">
			{groups?.map((group, index) => <OperationGroup key={index} group={group} />)}
		</ul>
	);
};

const OperationGroup: React.FC<{ group: MessageGroup }> = ({ group }) => {
	const [header, contents] = (() => {
		switch (group.variant) {
			case 'Shared': {
				const header = (
					<div className="flex items-center space-x-2 p-2">
						<span>{group.model}</span>
						<span className="">{group.id}</span>
					</div>
				);
				const contents = (
					<ul className="flex flex-col space-y-2 p-2">
						{group.messages.map((message, index) => (
							<li key={index} className="flex flex-row justify-between px-2">
								{typeof message.op.data === 'string' ? (
									<p>{message.op.data === 'c' ? 'Create' : 'Delete'}</p>
								) : (
									<p>Update - {message.op.data.u.field}</p>
								)}
								<p className="text-gray-400">{message.timestamp}</p>
							</li>
						))}
					</ul>
				);
				return [header, contents];
			}
			case 'Relation': {
				const header = (
					<div className="flex items-center space-x-2 p-2">
						<span>{group.relation}</span>
						<span className="">{group.item_id}</span>
						<span className="">in</span>
						<span className="">{group.group_id}</span>
					</div>
				);

				const contents = (
					<ul className="flex flex-col space-y-2 p-2">
						{group.messages.map((message, index) => (
							<li key={index} className="flex flex-row justify-between px-2">
								{typeof message.op.data === 'string' ? (
									<p>{message.op.data === 'c' ? 'Create' : 'Delete'}</p>
								) : (
									<p>Update - {message.op.data.u.field}</p>
								)}
								<p className="text-gray-400">{message.timestamp}</p>
							</li>
						))}
					</ul>
				);

				return [header, contents];
			}
		}
	})();

	return (
		<div className="divide-y divide-gray bg-app-darkBox">
			{header}
			{contents}
		</div>
	);
};

function calculateGroups(messages: CRDTOperation[]) {
	return messages.reduce<MessageGroup[]>((acc, curr) => {
		const { typ } = curr;

		if ('model' in typ) {
			const id = stringify(typ.record_id.pub_id);

			const latest = (() => {
				const latest = acc[acc.length - 1];

				if (
					!latest ||
					latest.variant !== 'Shared' ||
					latest.model !== typ.model ||
					latest.id !== id
				) {
					const group: MessageGroup = {
						variant: 'Shared',
						model: typ.model,
						id,
						messages: []
					};

					acc.push(group);

					return group;
				} else {
					return latest;
				}
			})();

			latest.messages.push({
				op: typ,
				timestamp: curr.timestamp
			});
		} else {
			const id = {
				item: stringify(typ.relation_item.pub_id),
				group: stringify(typ.relation_group.pub_id)
			};

			const latest = (() => {
				const latest = acc[acc.length - 1];

				if (
					!latest ||
					latest.variant !== 'Relation' ||
					latest.relation !== typ.relation ||
					latest.item_id !== id.item ||
					latest.group_id !== id.group
				) {
					const group: MessageGroup = {
						variant: 'Relation',
						relation: typ.relation,
						item_id: id.item,
						group_id: id.group,
						messages: []
					};

					acc.push(group);

					return group;
				} else {
					return latest;
				}
			})();

			latest.messages.push({
				op: typ,
				timestamp: curr.timestamp
			});
		}

		return acc;
	}, []);
}
