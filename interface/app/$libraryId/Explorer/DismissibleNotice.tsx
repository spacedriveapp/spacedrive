import { Image, Video } from '@sd/assets/icons';
import { Icon, X } from 'phosphor-react';
import { Button } from '@sd/ui';

interface DismissibleNoticeProps {
	onClose?: () => void;
	title: string | JSX.Element;
	description: string;
	icon: Icon;
}

export default function DismissibleNotice({
	onClose,
	title,
	description,
	icon: Icon
}: DismissibleNoticeProps) {
	return (
		<div className="m-5 rounded-md bg-gradient-to-l from-accent-deep via-accent-faint to-purple-500 p-1">
			<div className="back relative flex h-full w-full flex-col  rounded bg-app py-4 px-3">
				{/* <div className="absolute right-0 top-2 mr-2">
					<Button variant="outline" size="icon" onClick={onClose}>
						<X className="h-4 w-4" />
					</Button>
				</div> */}
				<div className="flex items-center">
					<div className="relative mr-10 ml-3 h-14 w-14 flex-shrink-0">
						<img
							src={Image}
							className="absolute left-6 -top-1 h-14 w-14 rotate-6 overflow-hidden"
						/>
						<img
							src={Video}
							className="absolute top-2 z-10 h-14 w-14 -rotate-6 overflow-hidden"
						/>
						{/* <Icon weight="fill" size={50} className="text-white" /> */}
					</div>
					<div className="flex flex-col justify-center">
						<h1 className="text-xl font-bold text-white">{title}</h1>
						<div className="text-xs text-white/70">{description} </div>
					</div>
					<div className="flex-1" />
					<div className="mx-3 flex-shrink-0 space-x-2">
						<Button
							variant="outline"
							className="border-white/10 font-medium hover:border-white/20"
							onClick={() => console.log('Learn More')}
						>
							Learn More
						</Button>
						<Button
							variant="accent"
							className="font-medium"
							onClick={() => console.log('Learn More')}
						>
							Got it
						</Button>
					</div>
				</div>
			</div>
		</div>
	);
}

// export default function DismissibleNotice({
// 	onClose,
// 	title,
// 	description,
// 	icon: Icon
// }: DismissibleNoticeProps) {
// 	return (
// 		<div className="m-5 rounded-md bg-gradient-to-r from-accent-deep via-accent-faint to-purple-500 p-1">
// 			<div className="back relative flex h-full w-full flex-col justify-center rounded bg-app py-5 px-4">
// 				<div className="absolute right-0 top-2 mr-2">
// 					<Button variant="outline" size="icon" onClick={onClose}>
// 						<X className="h-4 w-4" />
// 					</Button>
// 				</div>
// 				<div className="flex flex-col items-center">
// 					<div className="mr-5 ml-3">
// 						<Icon weight="fill" size={50} className="text-white" />
// 					</div>
// 					<h1 className="text-2xl font-black text-white">{title}</h1>
// 					<div className="text-center text-sm text-white/70">{description} </div>
// 					<div className="flex flex-row space-x-2">
// 						<Button
// 							variant="gray"
// 							className="mt-3 font-medium"
// 							onClick={() => console.log('Learn More')}
// 						>
// 							Learn More
// 						</Button>
// 						<Button
// 							variant="accent"
// 							className="mt-3 font-medium"
// 							onClick={() => console.log('Learn More')}
// 						>
// 							Get started
// 						</Button>
// 					</div>
// 				</div>
// 			</div>
// 		</div>
// 	);
// }
