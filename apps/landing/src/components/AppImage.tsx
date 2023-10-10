import Image from 'next/image';
import { tw } from '@sd/ui';

const AppFrameOuter = tw.div`relative m-auto flex w-full max-w-7xl rounded-lg transition-opacity px-4`;
const AppFrameInner = tw.div`z-30 flex w-full rounded-lg border-t border-app-line/50 backdrop-blur`;

const AppImage = () => {
	return (
		<div className="w-screen">
			<div className="relative mx-auto max-w-full sm:w-full sm:max-w-[1400px]">
				<div className="bloom burst bloom-one" />
				<div className="bloom burst bloom-three" />
				<div className="bloom burst bloom-two" />
			</div>
			<div className="fade-in-app-embed relative z-30 mt-8 h-[255px] w-full px-1 text-center sm:mt-16 sm:h-[428px] md:h-[428px] lg:h-[628px]">
				<AppFrameOuter>
					<AppFrameInner>
						<Image
							className="rounded-lg"
							alt="spacedrive"
							src="/images/app.webp"
							loading="eager"
							width={1278}
							height={626}
							quality={100}
						/>
					</AppFrameInner>
				</AppFrameOuter>
			</div>
		</div>
	);
};

export default AppImage;
