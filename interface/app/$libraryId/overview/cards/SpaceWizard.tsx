import { Desktop, FilePlus, MagicWand } from '@phosphor-icons/react';
import { useNavigate } from 'react-router';
import { Device, HardwareModel, useLibraryQuery } from '@sd/client';
import { Button, buttonStyles, TextArea, Tooltip } from '@sd/ui';
import { Icon, Icon as SdIcon } from '~/components';
import { useLocale } from '~/hooks';
import { hardwareModelAsNumberToIcon, hardwareModelToIcon } from '~/util/hardware';

import StarfieldEffect from '../../peer/StarfieldEffect';

const SpaceWizard = () => {
	const navigate = useNavigate();
	const { t } = useLocale();

	return (
		<>
			<div className="relative grid grid-cols-1 gap-2">
				<TextArea
					className="!rounded-xl !p-4 text-[1.05rem] font-medium text-ink-dull"
					placeholder="How would you like to optimize your storage?"
				/>

				<div className="space-between mt-2 flex items-start gap-2">
					{/* File dropzone */}
					<div className="relative flex h-28 grow items-center justify-center rounded-lg border border-dotted border-app-line text-ink-dull">
						{/* <StarfieldEffect className="!bg-transparent" /> */}
						<div className="pointer-events-none absolute inset-0 flex items-center justify-center">
							{t('Drop anything here as context')}
						</div>
					</div>
					<div className="flex items-start justify-end gap-2">
						<Button variant="gray" className="flex items-center gap-2">
							Chat
						</Button>
						<Button variant="accent" className="flex items-center gap-2">
							<MagicWand size={16} weight="fill" />
							Plan
						</Button>
					</div>
				</div>
			</div>
		</>
	);
};

export default SpaceWizard;
