import { Stack } from 'expo-router';

export default function DrawerLayout() {
  return (
    <Stack screenOptions={{ headerShown: false }}>
      <Stack.Screen name="(tabs)" />
      <Stack.Screen
        name="explorer"
        options={{
          animation: 'slide_from_right',
          animationDuration: 200,
        }}
      />
    </Stack>
  );
}
