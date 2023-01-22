import { ReactComponent as CaretDown } from '@sd/assets/svgs/caret.svg';
import { Button, ScreenHeading, tw } from '@sd/ui';
import clsx from 'clsx';
import { ArrowArcLeft, X } from 'phosphor-react';
import { PropsWithChildren } from 'react';
import { useNavigate } from 'react-router';

import { useOperatingSystem } from '../../hooks/useOperatingSystem';
import { Divider } from '../explorer/inspector/Divider';

interface Props extends PropsWithChildren {
	title: string;
	topRight?: React.ReactNode;
}

const PageOuter = tw.div`flex h-screen flex-col m-3`;
const Page = tw.div`flex-1 w-full border rounded-md shadow-md shadow-app-shade/30 border-app-box bg-app-box/20`;
const PageInner = tw.div`flex flex-col w-full h-screen max-w-4xl py-6`;
const HeaderArea = tw.div`flex flex-row px-8 items-center space-x-4 mb-2`;
const ContentContainer = tw.div`max-w-2xl px-8 pt-5 -mt-1 space-y-6 custom-scroll page-scroll`;

export const SettingsSubPage = ({ children, title, topRight }: Props) => {
	const navigate = useNavigate();

	return (
		<PageOuter>
			<div data-tauri-drag-region className="absolute w-full h-5" />
			<Page>
				<PageInner>
					<HeaderArea>
						<Button variant="outline" size="icon" onClick={() => navigate(-1)}>
							<div className="flex justify-center w-4 h-4">
								<CaretDown
									className="w-[12px] text-ink-dull transition-transform rotate-90 translate-x-[-1px]"
									aria-hidden="true"
								/>
							</div>
						</Button>
						<h3 className="flex-grow text-lg font-semibold">{title}</h3>
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
};
