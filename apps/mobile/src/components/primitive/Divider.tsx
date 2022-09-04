import React from 'react';
import { StyleProp, Text, View, ViewStyle } from 'react-native';
import tw from '~/lib/tailwind';

type DividerProps = {
	style?: StyleProp<ViewStyle>;
};

const Divider = ({ style }: DividerProps) => {
	return <View style={[tw`w-full my-1 h-[1px] bg-gray-550`, style]} />;
};

export default Divider;
