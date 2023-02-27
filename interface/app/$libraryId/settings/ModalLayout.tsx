import { CaretDown } from 'phosphor-react';
import { PropsWithChildren } from 'react';
import { useNavigate } from 'react-router';
import { Button, Divider, tw } from '@sd/ui';

interface Props extends PropsWithChildren {
	title: string;
	topRight?: React.ReactNode;
}

const PageOuter = tw.div`flex h-screen flex-col m-3 -mt-4`;
const Page = tw.div`flex-1 w-full border rounded-md shadow-md shadow-app-shade/30 border-app-box bg-app-box/20`;
const PageInner = tw.div`flex flex-col max-w-4xl w-full h-screen py-6`;
const HeaderArea = tw.div`flex flex-row px-8 items-center space-x-4 mb-2`;
const ContentContainer = tw.div`px-8 pt-5 -mt-1 space-y-6 custom-scroll page-scroll`;

export default ({ children, title, topRight }: Props) => (
	<PageOuter>
		<Page>
			<PageInner>
				<HeaderArea>
					<BackButton />
					<h3 className="grow text-lg font-semibold">{title}</h3>
					{topRight}
				</HeaderArea>
				<div className="px-8">
					<Divider />
				</div>
				<ContentContainer>{children}</ContentContainer>
			</PageInner>
		</Page>
	</PageOuter>
);

const BackButton = () => {
	const navigate = useNavigate();

	return (
		<Button variant="outline" size="icon" onClick={() => navigate(-1)}>
			<div className="flex h-4 w-4 justify-center">
				<CaretDown
					className="text-ink-dull w-[12px] translate-x-[-1px] rotate-90 transition-transform"
					aria-hidden="true"
				/>
			</div>
		</Button>
	);
};
