import Clipboard from "@react-native-clipboard/clipboard";
import { CameraView, useCameraPermissions } from "expo-camera";
import React, { useEffect, useState } from "react";
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
import QRCode from "react-native-qrcode-svg";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { useCoreAction, useCoreQuery } from "../client";

interface PairingPanelProps {
  isOpen: boolean;
  onClose: () => void;
  initialMode?: "generate" | "join";
}

export function PairingPanel({
  isOpen,
  onClose,
  initialMode = "generate",
}: PairingPanelProps) {
  const insets = useSafeAreaInsets();
  const [mode, setMode] = useState<"generate" | "join">(initialMode);
  const [joinCode, setJoinCode] = useState("");
  const [joinNodeId, setJoinNodeId] = useState("");
  const [showScanner, setShowScanner] = useState(false);
  const [permission, requestPermission] = useCameraPermissions();

  const generatePairing = useCoreAction("network.pair.generate");
  const joinPairing = useCoreAction("network.pair.join");
  const cancelPairing = useCoreAction("network.pair.cancel");

  const { data: pairingStatus, refetch: refetchStatus } = useCoreQuery(
    "network.pair.status",
    null
  );

  // Poll status when in active pairing
  useEffect(() => {
    if (!isOpen) return;

    const interval = setInterval(() => {
      refetchStatus();
    }, 1000);

    return () => clearInterval(interval);
  }, [isOpen, refetchStatus]);

  const currentSession = pairingStatus?.sessions?.[0];

  const handleGenerate = () => {
    generatePairing.mutate({});
  };

  const handleJoin = () => {
    if (!joinCode.trim()) return;
    joinPairing.mutate({
      code: joinCode,
      node_id: joinNodeId.trim() || null,
    });
  };

  const handleCancel = () => {
    if (currentSession) {
      cancelPairing.mutate({ session_id: currentSession.id });
    }
    generatePairing.reset();
    joinPairing.reset();
    setJoinCode("");
    setJoinNodeId("");
    setShowScanner(false);
  };

  const handleClose = () => {
    handleCancel();
    onClose();
  };

  const copyCode = () => {
    if (generatePairing.data?.code) {
      Clipboard.setString(generatePairing.data.code);
      Alert.alert("Copied", "Pairing code copied to clipboard");
    }
  };

  const handleQRScan = (data: string) => {
    try {
      const parsed = JSON.parse(data);
      // QR code contains: { version, words, node_id, relay_url, session_id }
      if (parsed.words || parsed.code) {
        console.log(
          "[PairingPanel] QR scanned, auto-joining with full QR data"
        );
        const words = parsed.words || parsed.code;
        setJoinCode(words);
        setShowScanner(false);
        // Pass the ENTIRE QR JSON as the code for internet pairing
        joinPairing.mutate({
          code: data, // Full QR JSON string
          node_id: parsed.node_id || null,
        });
      }
    } catch {
      // If not JSON, treat as plain word code (local pairing)
      console.log(
        "[PairingPanel] QR scanned, auto-joining with plain code:",
        data
      );
      setJoinCode(data);
      setShowScanner(false);
      // Just the words for local pairing
      joinPairing.mutate({
        code: data,
        node_id: null,
      });
    }
  };

  const openScanner = async () => {
    if (!permission?.granted) {
      const result = await requestPermission();
      if (!result.granted) {
        Alert.alert(
          "Camera Permission",
          "Camera access is required to scan QR codes"
        );
        return;
      }
    }
    setShowScanner(true);
  };

  const isCompleted =
    currentSession?.state === "Completed" || joinPairing.isSuccess;

  useEffect(() => {
    if (isCompleted) {
      const timer = setTimeout(() => {
        handleClose();
      }, 2000);
      return () => clearTimeout(timer);
    }
  }, [isCompleted]);

  if (!isOpen) return null;

  return (
    <Modal
      animationType="slide"
      onRequestClose={handleClose}
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
                  Device Pairing
                </Text>
                <Text className="mt-0.5 text-ink-dull text-xs">
                  Connect another device to share files
                </Text>
              </View>
              <Pressable
                className="rounded-lg p-2 active:bg-app-hover"
                onPress={handleClose}
              >
                <Text className="text-ink-dull text-xl">✕</Text>
              </Pressable>
            </View>
          </View>

          {/* Mode Tabs */}
          <View className="flex-row border-app-line border-b">
            <Pressable
              className={`flex-1 px-6 py-3 ${
                mode === "generate" ? "border-accent border-b-2" : ""
              }`}
              onPress={() => setMode("generate")}
            >
              <Text
                className={`text-center font-medium text-sm ${
                  mode === "generate" ? "text-accent" : "text-ink-dull"
                }`}
              >
                Generate Code
              </Text>
            </Pressable>
            <Pressable
              className={`flex-1 px-6 py-3 ${
                mode === "join" ? "border-accent border-b-2" : ""
              }`}
              onPress={() => setMode("join")}
            >
              <Text
                className={`text-center font-medium text-sm ${
                  mode === "join" ? "text-accent" : "text-ink-dull"
                }`}
              >
                Join with Code
              </Text>
            </Pressable>
          </View>

          {/* Content */}
          <ScrollView
            className="flex-1"
            contentContainerStyle={{
              padding: 24,
              paddingBottom: insets.bottom + 24,
            }}
          >
            {mode === "generate" ? (
              <GenerateMode
                currentSession={currentSession}
                generatePairing={generatePairing}
                onCancel={handleCancel}
                onCopyCode={copyCode}
                onGenerate={handleGenerate}
              />
            ) : showScanner ? (
              <ScannerMode
                onClose={() => setShowScanner(false)}
                onScan={handleQRScan}
              />
            ) : (
              <JoinMode
                currentSession={currentSession}
                joinCode={joinCode}
                joinNodeId={joinNodeId}
                joinPairing={joinPairing}
                onCancel={handleCancel}
                onJoin={handleJoin}
                onOpenScanner={openScanner}
                setJoinCode={setJoinCode}
                setJoinNodeId={setJoinNodeId}
              />
            )}

            {/* Success State */}
            {isCompleted && (
              <View className="mt-6 flex-row items-center gap-3 rounded-lg border border-accent/30 bg-accent/10 p-4">
                <Text className="text-accent text-xl">✓</Text>
                <View className="flex-1">
                  <Text className="font-medium text-accent text-sm">
                    Pairing successful!
                  </Text>
                  <Text className="mt-0.5 text-ink-dull text-xs">
                    {joinPairing.data
                      ? `Connected to ${joinPairing.data.device_name}`
                      : "Device paired"}
                  </Text>
                </View>
              </View>
            )}
          </ScrollView>
        </View>
      </View>
    </Modal>
  );
}

function GenerateMode({
  generatePairing,
  currentSession,
  onGenerate,
  onCancel,
  onCopyCode,
}: any) {
  const hasCode = generatePairing.data?.code;
  const isLoading = generatePairing.isPending;
  const state = currentSession?.state;

  return (
    <View className="gap-6">
      {hasCode ? (
        <>
          {/* QR Code */}
          <View className="items-center gap-3">
            <Text className="text-ink-dull text-sm">
              Scan with mobile device
            </Text>
            <View className="rounded-lg bg-white p-4">
              <QRCode size={200} value={generatePairing.data.qr_json} />
            </View>
          </View>

          {/* Word Code */}
          <View>
            <Text className="mb-2 font-medium text-ink-dull text-xs uppercase tracking-wider">
              Or type manually:
            </Text>
            <View className="rounded-lg border border-sidebar-line bg-sidebar-box p-4">
              <Text className="font-mono text-ink text-sm">
                {generatePairing.data.code}
              </Text>
            </View>
            <Pressable
              className="mt-2 rounded-lg border border-app-line bg-app-box p-2 active:bg-app-hover"
              onPress={onCopyCode}
            >
              <Text className="text-center text-ink-dull text-sm">
                Copy Code
              </Text>
            </Pressable>
          </View>

          {/* Status */}
          {state && (
            <View className="flex-row items-center gap-2 rounded-lg border border-app-line bg-app-box/40 p-3">
              <View className="h-2 w-2 rounded-full bg-accent" />
              <Text className="text-ink-dull text-sm">
                {state === "Broadcasting" || state === "WaitingForConnection"
                  ? "Waiting for device to connect..."
                  : state === "Authenticating"
                    ? "Authenticating device..."
                    : state === "ExchangingKeys"
                      ? "Exchanging encryption keys..."
                      : "Ready to pair"}
              </Text>
            </View>
          )}

          {/* Cancel Button */}
          <Pressable
            className="rounded-lg border border-app-line bg-app-box px-4 py-2.5 active:bg-app-hover"
            onPress={onCancel}
          >
            <Text className="text-center font-medium text-ink-dull text-sm">
              Cancel
            </Text>
          </Pressable>
        </>
      ) : (
        <>
          {/* Info */}
          <View className="flex-row gap-3 rounded-lg border border-sidebar-line bg-sidebar-box/40 p-4">
            <View className="h-10 w-10 items-center justify-center rounded-full bg-accent/10">
              <Text className="text-accent text-xl">◊</Text>
            </View>
            <View className="flex-1">
              <Text className="font-medium text-ink text-sm">How it works</Text>
              <Text className="mt-1 text-ink-dull text-xs">
                Generate a secure code to share with another device. They'll
                scan or enter the code to connect.
              </Text>
            </View>
          </View>

          {/* Generate Button */}
          <Pressable
            className={`flex-row items-center justify-center gap-2 rounded-lg px-4 py-3 ${
              isLoading ? "bg-accent/50" : "bg-accent active:bg-accent/90"
            }`}
            disabled={isLoading}
            onPress={onGenerate}
          >
            {isLoading && <ActivityIndicator color="white" size="small" />}
            <Text className="font-medium text-white">
              {isLoading ? "Generating..." : "Generate Pairing Code"}
            </Text>
          </Pressable>
        </>
      )}

      {/* Error */}
      {generatePairing.isError && (
        <View className="flex-row gap-2 rounded-lg border border-red-500/30 bg-red-500/10 p-3">
          <Text className="text-red-500 text-xl">⚠</Text>
          <View className="flex-1">
            <Text className="font-medium text-red-500 text-sm">
              Failed to generate code
            </Text>
            <Text className="mt-0.5 text-ink-dull text-xs">
              {String(generatePairing.error)}
            </Text>
          </View>
        </View>
      )}
    </View>
  );
}

function JoinMode({
  joinCode,
  setJoinCode,
  joinNodeId,
  setJoinNodeId,
  joinPairing,
  currentSession,
  onJoin,
  onCancel,
  onOpenScanner,
}: any) {
  const isLoading = joinPairing.isPending;
  const state = currentSession?.state;

  return (
    <View className="gap-6">
      {/* Instructions */}
      <View className="flex-row gap-3 rounded-lg border border-sidebar-line bg-sidebar-box/40 p-4">
        <View className="h-10 w-10 items-center justify-center rounded-full bg-accent/10">
          <Text className="text-accent text-xl">◊</Text>
        </View>
        <View className="flex-1">
          <Text className="font-medium text-ink text-sm">
            Enter pairing code
          </Text>
          <Text className="mt-1 text-ink-dull text-xs">
            Scan the QR code or enter the 12-word code from the other device.
          </Text>
        </View>
      </View>

      {/* QR Scanner Button */}
      <Pressable
        className={`flex-row items-center justify-center gap-2 rounded-lg border-2 border-dashed px-4 py-3 ${
          isLoading || state
            ? "border-app-line bg-app-box/50"
            : "border-accent bg-accent/10 active:bg-accent/20"
        }`}
        disabled={isLoading || !!state}
        onPress={onOpenScanner}
      >
        <Text
          className={`text-xl ${
            isLoading || state ? "text-ink-faint" : "text-accent"
          }`}
        >
          📷
        </Text>
        <Text
          className={`font-medium ${
            isLoading || state ? "text-ink-faint" : "text-accent"
          }`}
        >
          Scan QR Code
        </Text>
      </Pressable>

      {/* Code Input */}
      <View>
        <Text className="mb-2 font-medium text-ink-dull text-xs uppercase tracking-wider">
          Or Enter Code Manually
        </Text>
        <TextInput
          className="rounded-lg border border-sidebar-line bg-sidebar-box px-4 py-3 text-ink text-sm"
          editable={!(isLoading || state)}
          multiline
          numberOfLines={3}
          onChangeText={setJoinCode}
          placeholder="brave-lion-sunset-river-eagle-mountain..."
          placeholderTextColor="hsl(235, 10%, 55%)"
          style={{ textAlignVertical: "top" }}
          value={joinCode}
        />
        <Text className="mt-2 text-ink-dull text-xs">
          Paste the full code or type the 12 words separated by hyphens
        </Text>
      </View>

      {/* Node ID Input */}
      <View>
        <Text className="mb-2 font-medium text-ink-dull text-xs uppercase tracking-wider">
          Node ID <Text className="text-ink-faint">(optional)</Text>
        </Text>
        <TextInput
          className="rounded-lg border border-sidebar-line bg-sidebar-box px-4 py-2.5 font-mono text-ink text-xs"
          editable={!(isLoading || state)}
          onChangeText={setJoinNodeId}
          placeholder="For cross-network pairing"
          placeholderTextColor="hsl(235, 10%, 55%)"
          value={joinNodeId}
        />
      </View>

      {/* Status */}
      {state && (
        <View className="flex-row items-center gap-2 rounded-lg border border-app-line bg-app-box/40 p-3">
          <View className="h-2 w-2 rounded-full bg-accent" />
          <Text className="text-ink-dull text-sm">
            {state === "Scanning" || state === "Connecting"
              ? "Finding device..."
              : state === "Authenticating"
                ? "Authenticating..."
                : state === "ExchangingKeys"
                  ? "Exchanging keys..."
                  : state === "EstablishingSession"
                    ? "Establishing secure session..."
                    : "Processing..."}
          </Text>
        </View>
      )}

      {/* Action Buttons */}
      <View className="flex-row gap-3">
        {state ? (
          <Pressable
            className="flex-1 rounded-lg border border-app-line bg-app-box px-4 py-2.5 active:bg-app-hover"
            onPress={onCancel}
          >
            <Text className="text-center font-medium text-ink-dull text-sm">
              Cancel
            </Text>
          </Pressable>
        ) : (
          <>
            <Pressable
              className="rounded-lg border border-app-line bg-app-box px-6 py-2.5 active:bg-app-hover"
              onPress={onCancel}
            >
              <Text className="text-center font-medium text-ink-dull text-sm">
                Clear
              </Text>
            </Pressable>
            <Pressable
              className={`flex-1 flex-row items-center justify-center gap-2 rounded-lg px-4 py-2.5 ${
                isLoading || !joinCode.trim()
                  ? "bg-accent/50"
                  : "bg-accent active:bg-accent/90"
              }`}
              disabled={isLoading || !joinCode.trim()}
              onPress={onJoin}
            >
              {isLoading && <ActivityIndicator color="white" size="small" />}
              <Text className="font-medium text-white">
                {isLoading ? "Joining..." : "Join"}
              </Text>
            </Pressable>
          </>
        )}
      </View>

      {/* Error */}
      {joinPairing.isError && (
        <View className="flex-row gap-2 rounded-lg border border-red-500/30 bg-red-500/10 p-3">
          <Text className="text-red-500 text-xl">⚠</Text>
          <View className="flex-1">
            <Text className="font-medium text-red-500 text-sm">
              Failed to join
            </Text>
            <Text className="mt-0.5 text-ink-dull text-xs">
              {String(joinPairing.error)}
            </Text>
          </View>
        </View>
      )}
    </View>
  );
}

function ScannerMode({
  onScan,
  onClose,
}: {
  onScan: (data: string) => void;
  onClose: () => void;
}) {
  const [scanned, setScanned] = React.useState(false);

  const handleBarCodeScanned = ({ data }: { data: string }) => {
    if (scanned) return;
    console.log("[Scanner] QR code scanned:", data.substring(0, 50));
    setScanned(true);
    onScan(data);
  };

  return (
    <View className="-mx-6 -my-6 flex-1">
      <CameraView
        barcodeScannerSettings={{
          barcodeTypes: ["qr"],
        }}
        facing="back"
        onBarcodeScanned={scanned ? undefined : handleBarCodeScanned}
        style={{ flex: 1 }}
      >
        <View className="flex-1 items-center justify-center">
          <View className="h-64 w-64 rounded-lg border-2 border-accent" />
          <Text className="mt-4 px-6 text-center text-white">
            {scanned ? "Scanned! Processing..." : "Point camera at QR code"}
          </Text>
        </View>
      </CameraView>

      <Pressable
        className="absolute top-4 right-4 rounded-full bg-black/50 p-3"
        onPress={onClose}
      >
        <Text className="text-white text-xl">✕</Text>
      </Pressable>
    </View>
  );
}
