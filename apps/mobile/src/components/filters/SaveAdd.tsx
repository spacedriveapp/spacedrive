import { AnimatePresence, MotiView } from 'moti';
import { Text } from 'react-native';
import { Button } from '~/components/primitive/Button';
import { tw, twStyle } from '~/lib/tailwind';
import { useSearchStore } from '~/stores/searchStore';

export const SaveAdd = () => {
	const searchStore = useSearchStore();
	return (
		<AnimatePresence>
			{searchStore.showActionButtons && (
				<MotiView
					from={{ translateY: 100 }}
					animate={{ translateY: 0 }}
					transition={{ type: 'timing', duration: 300 }}
					exit={{ translateY: 100 }}
					style={twStyle(
						`flex-row justify-between gap-2 border-t border-app-line/50 h-[100px] pt-5 px-6 bg-mobile-header`,
						{
							position: 'fixed'
						}
					)}
				>
					<Button style={tw`flex-1 h-10`} variant="dashed">
						<Text style={tw`font-bold text-ink-dull`}>+ Save search</Text>
					</Button>
					<Button style={tw`flex-1 h-10`} variant="accent">
						<Text style={tw`font-bold text-ink`}>+ Add filters</Text>
					</Button>
				</MotiView>
			)}
		</AnimatePresence>
	);
};
