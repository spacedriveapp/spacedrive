import { useState } from "react";
import {
  ActivityIndicator,
  Alert,
  Modal,
  Pressable,
  ScrollView,
  Text,
  TextInput,
  View,
} from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { useCoreAction, useCoreQuery, useSpacedriveClient } from "../client";
import { useSidebarStore } from "../stores";

interface LibrarySwitcherPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

export function LibrarySwitcherPanel({
  isOpen,
  onClose,
}: LibrarySwitcherPanelProps) {
  const insets = useSafeAreaInsets();
  const client = useSpacedriveClient();
  const { currentLibraryId, setCurrentLibrary: setStoreLibrary } =
    useSidebarStore();
  const [showCreateForm, setShowCreateForm] = useState(false);
  const [newLibraryName, setNewLibraryName] = useState("");

  const { data: libraries, refetch } = useCoreQuery("libraries.list", {
    include_stats: false,
  });

  const createLibrary = useCoreAction("libraries.create");
  const deleteLibrary = useCoreAction("libraries.delete");

  const handleSelectLibrary = (libraryId: string) => {
    console.log("[LibrarySwitcher] Selecting library:", libraryId);
    setStoreLibrary(libraryId);
    client.setCurrentLibrary(libraryId);
    onClose();
  };

  const handleCreateLibrary = async () => {
    if (!newLibraryName.trim()) return;

    try {
      const result = await createLibrary.mutateAsync({
        name: newLibraryName,
        path: null,
      });

      setNewLibraryName("");
      setShowCreateForm(false);
      refetch();

      // Auto-select the new library
      if (result?.id) {
        handleSelectLibrary(result.id);
      }
    } catch (error) {
      Alert.alert("Error", "Failed to create library");
    }
  };

  const handleDeleteLibrary = (libraryId: string, libraryName: string) => {
    Alert.alert(
      "Delete Library",
      `Are you sure you want to delete "${libraryName}"? This action cannot be undone.`,
      [
        { text: "Cancel", style: "cancel" },
        {
          text: "Delete",
          style: "destructive",
          onPress: async () => {
            try {
              await deleteLibrary.mutateAsync({ id: libraryId });
              refetch();

              // If deleted library was selected, switch to first available
              if (libraryId === currentLibraryId) {
                const remaining = (libraries || []).filter(
                  (lib: any) => lib.id !== libraryId
                );
                if (remaining.length > 0) {
                  handleSelectLibrary(remaining[0].id);
                } else {
                  setStoreLibrary(null);
                  client.setCurrentLibrary(null);
                }
              }
            } catch (error) {
              Alert.alert("Error", "Failed to delete library");
            }
          },
        },
      ]
    );
  };

  if (!isOpen) return null;

  return (
    <Modal
      animationType="slide"
      onRequestClose={onClose}
      transparent
      visible={isOpen}
    >
      <View className="flex-1 bg-black/50">
        <View
          className="flex-1 overflow-hidden rounded-t-3xl bg-app-box"
          style={{ marginTop: insets.top + 40 }}
        >
          {/* Header */}
          <View className="border-app-line border-b px-6 py-4">
            <View className="flex-row items-center justify-between">
              <View>
                <Text className="font-semibold text-ink text-lg">
                  Libraries
                </Text>
                <Text className="mt-0.5 text-ink-dull text-xs">
                  Switch or manage your libraries
                </Text>
              </View>
              <Pressable
                className="rounded-lg p-2 active:bg-app-hover"
                onPress={onClose}
              >
                <Text className="text-ink-dull text-xl">✕</Text>
              </Pressable>
            </View>
          </View>

          {/* Content */}
          <ScrollView
            className="flex-1"
            contentContainerStyle={{
              padding: 16,
              paddingBottom: insets.bottom + 16,
            }}
          >
            {/* Libraries List */}
            <View className="mb-4 gap-2">
              {libraries &&
                Array.isArray(libraries) &&
                libraries.map((lib: any) => (
                  <View
                    className={`rounded-lg border p-4 ${
                      currentLibraryId === lib.id
                        ? "border-accent/30 bg-accent/10"
                        : "border-app-line bg-app-darkBox"
                    }`}
                    key={lib.id}
                  >
                    <Pressable
                      className="flex-1"
                      onPress={() => handleSelectLibrary(lib.id)}
                    >
                      <View className="flex-row items-center justify-between">
                        <View className="flex-1">
                          <Text
                            className={`font-semibold text-base ${
                              currentLibraryId === lib.id
                                ? "text-accent"
                                : "text-ink"
                            }`}
                          >
                            {lib.name}
                          </Text>
                          {lib.description && (
                            <Text className="mt-0.5 text-ink-dull text-xs">
                              {lib.description}
                            </Text>
                          )}
                          {currentLibraryId === lib.id && (
                            <Text className="mt-1 text-accent text-xs">
                              ✓ Currently active
                            </Text>
                          )}
                        </View>
                        {currentLibraryId !== lib.id && (
                          <Pressable
                            className="ml-2 rounded-lg p-2 active:bg-app-hover"
                            onPress={() =>
                              handleDeleteLibrary(lib.id, lib.name)
                            }
                          >
                            <Text className="text-lg text-red-500">🗑</Text>
                          </Pressable>
                        )}
                      </View>
                    </Pressable>
                  </View>
                ))}
            </View>

            {/* Create Library Section */}
            {showCreateForm ? (
              <View className="gap-4 rounded-lg border border-app-line bg-app-darkBox p-4">
                <View>
                  <Text className="mb-2 font-medium text-ink text-sm">
                    Library Name
                  </Text>
                  <TextInput
                    autoFocus
                    className="rounded-lg border border-sidebar-line bg-sidebar-box px-4 py-3 text-ink"
                    onChangeText={setNewLibraryName}
                    placeholder="My Photos"
                    placeholderTextColor="hsl(235, 10%, 55%)"
                    value={newLibraryName}
                  />
                </View>

                <View className="flex-row gap-3">
                  <Pressable
                    className="flex-1 rounded-lg border border-app-line bg-app-box px-4 py-2.5 active:bg-app-hover"
                    onPress={() => {
                      setShowCreateForm(false);
                      setNewLibraryName("");
                    }}
                  >
                    <Text className="text-center font-medium text-ink-dull">
                      Cancel
                    </Text>
                  </Pressable>
                  <Pressable
                    className={`flex-1 flex-row items-center justify-center gap-2 rounded-lg px-4 py-2.5 ${
                      !newLibraryName.trim() || createLibrary.isPending
                        ? "bg-accent/50"
                        : "bg-accent active:bg-accent/90"
                    }`}
                    disabled={!newLibraryName.trim() || createLibrary.isPending}
                    onPress={handleCreateLibrary}
                  >
                    {createLibrary.isPending && (
                      <ActivityIndicator color="white" size="small" />
                    )}
                    <Text className="font-medium text-white">
                      {createLibrary.isPending ? "Creating..." : "Create"}
                    </Text>
                  </Pressable>
                </View>
              </View>
            ) : (
              <Pressable
                className="flex-row items-center justify-center gap-2 rounded-lg border-2 border-accent border-dashed bg-accent/5 p-4 active:bg-accent/10"
                onPress={() => setShowCreateForm(true)}
              >
                <Text className="text-accent text-xl">+</Text>
                <Text className="font-medium text-accent">
                  Create New Library
                </Text>
              </Pressable>
            )}
          </ScrollView>
        </View>
      </View>
    </Modal>
  );
}
