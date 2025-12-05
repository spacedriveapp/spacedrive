import { Platform } from 'react-native';
import { NativeTabs, Icon, Label } from 'expo-router/unstable-native-tabs';

export default function TabLayout() {
  return (
    <NativeTabs
      // CRITICAL: null background enables liquid glass on iOS!
      backgroundColor={Platform.OS === 'ios' ? null : 'hsl(235, 10%, 6%)'}
      disableTransparentOnScrollEdge={true}
      iconColor={Platform.OS === 'android' ? 'hsl(235, 10%, 55%)' : undefined}
      labelStyle={Platform.OS === 'android' ? {
        color: 'hsl(235, 10%, 55%)'
      } : undefined}
    >
      <NativeTabs.Trigger name="overview">
        <Label>Overview</Label>
        {Platform.OS === 'ios' ? (
          <Icon sf="square.grid.2x2" />
        ) : (
          <Icon name="grid" />
        )}
      </NativeTabs.Trigger>

      <NativeTabs.Trigger name="browse">
        <Label>Browse</Label>
        {Platform.OS === 'ios' ? (
          <Icon sf="folder" />
        ) : (
          <Icon name="folder" />
        )}
      </NativeTabs.Trigger>

      <NativeTabs.Trigger name="network">
        <Label>Network</Label>
        {Platform.OS === 'ios' ? (
          <Icon sf="network" />
        ) : (
          <Icon name="wifi" />
        )}
      </NativeTabs.Trigger>

      <NativeTabs.Trigger name="settings">
        <Label>Settings</Label>
        {Platform.OS === 'ios' ? (
          <Icon sf="gearshape" />
        ) : (
          <Icon name="settings" />
        )}
      </NativeTabs.Trigger>
    </NativeTabs>
  );
}
