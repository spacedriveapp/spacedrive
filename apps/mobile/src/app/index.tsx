import { Redirect } from "expo-router";

export default function Index() {
  // TODO: Check if user is onboarded, if not redirect to onboarding
  // For now, go straight to the main app
  return <Redirect href="/(drawer)/(tabs)/overview" />;
}
