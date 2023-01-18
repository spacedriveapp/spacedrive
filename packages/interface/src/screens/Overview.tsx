import { Card, tw } from '@sd/ui';

const GridCard = tw(Card)`h-[300px]`;

export default function OverviewScreen() {
	return (
		<div className="flex flex-col w-full h-screen p-5 custom-scroll page-scroll app-background">
			<div className="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-3">
				{/* <GridCard></GridCard>
				<GridCard></GridCard>
				<GridCard></GridCard>
				<GridCard></GridCard> */}
			</div>
		</div>
	);
}
