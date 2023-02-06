import { StyleProp, Text, View, ViewStyle } from 'react-native';
import tw from '~/lib/tailwind';

type DividerProps = {
	style?: StyleProp<ViewStyle>;
};

const Divider = ({ style }: DividerProps) => {
	return <View style={[tw`my-1 h-[1px] w-full bg-app-line`, style]} />;
};

export default Divider;
