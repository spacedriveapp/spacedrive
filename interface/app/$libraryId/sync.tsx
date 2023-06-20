import { CRDTOperation, useLibraryQuery, useLibrarySubscription } from '@sd/client';
import { tw } from '@sd/ui';

const Label = tw.span`text-gray-300`;
const Pill = tw.span`rounded-full bg-gray-500 px-2 py-1`;
const Row = tw.p`overflow-hidden text-ellipsis space-x-1`;

const OperationItem = ({ op }: { op: CRDTOperation }) => {
	let contents = null;

	if ('model' in op.typ) {
		let subContents = null;

		if (op.typ.data === 'd') {
			subContents = 'Delete';
		} else if ('c' in op.typ.data) {
			subContents = 'Create';
		} else {
			subContents = `Update - ${op.typ.data.u.field}`;
		}

		contents = (
			<>
				<div className="space-x-2">
					<Pill>{subContents}</Pill>
				</div>
				<Row>
					<Label>Model</Label>
					<span>{op.typ.model}</span>
				</Row>
				<Row>
					<Label>Time</Label>
					<span>{op.timestamp}</span>
				</Row>
			</>
		);
	}

	return <li className="space-y-1 rounded-md bg-gray-700 p-2 text-sm">{contents}</li>;
};

export const Component = () => {
	const messages = useLibraryQuery(['sync.messages']);

	useLibrarySubscription(['sync.newMessage'], {
		onData: () => messages.refetch()
	});

	return (
		<ul className="space-y-2">
			{messages.data?.map((op) => (
				<OperationItem key={op.id} op={op} />
			))}
		</ul>
	);
};
