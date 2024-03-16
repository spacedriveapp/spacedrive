import { DotsThreeOutlineVertical } from "phosphor-react-native";
import { View, Pressable, Text } from "react-native";
import { twStyle, tw } from "~/lib/tailwind";
import Card from "../layout/Card";
import { ClassInput } from "twrnc";
import { ModalRef } from "../layout/Modal";
import { Tag } from "@sd/client";

interface TagViewProps {
	tag: Tag
	tagStyle?: ClassInput;
	modalRef: React.RefObject<ModalRef>;
}

const GridTagView = ({tag, tagStyle, modalRef}: TagViewProps) => {
	return (
		<Card
			style={twStyle(
				`h-auto w-[90px] flex-col justify-center gap-3`,
				tagStyle
			)}
		>
			<View style={tw`flex-row items-center justify-between`}>
				<View
					style={twStyle('h-[28px] w-[28px] rounded-full', {
						backgroundColor: tag.color!
					})}
				/>
				<Pressable onPress={() => modalRef.current?.present()}>
					<DotsThreeOutlineVertical
						weight="fill"
						size={20}
						color={tw.color('ink-faint')}
					/>
				</Pressable>
			</View>
			<Text style={tw`w-full max-w-[75px] text-xs font-bold text-white`} numberOfLines={1}>
				{tag.name}
			</Text>
		</Card>
	);
}

export default GridTagView;
