import React, { useState, useEffect, useCallback } from "react";
import {
  View,
  Text,
  Modal,
  Pressable,
  TextInput,
  ScrollView,
  ActivityIndicator,
  Alert,
} from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { CameraView, useCameraPermissions } from "expo-camera";
import QRCode from "react-native-qrcode-svg";
import Clipboard from "@react-native-clipboard/clipboard";
import { useCoreAction, useCoreQuery } from "../client";

interface PairingPanelProps {
  isOpen: boolean;
  onClose: () => void;
  initialMode?: "generate" | "join";
}

interface ConfirmationRequest {
  sessionId: string;
  deviceName: string;
  deviceOs: string;
  confirmationCode: string;
  expiresAt: Date;
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
  const [confirmationInput, setConfirmationInput] = useState("");
  const [confirmationRequest, setConfirmationRequest] =
    useState<ConfirmationRequest | null>(null);
  const [timeRemaining, setTimeRemaining] = useState<number>(60);

  const generatePairing = useCoreAction("network.pair.generate");
  const joinPairing = useCoreAction("network.pair.join");
  const cancelPairing = useCoreAction("network.pair.cancel");
  const confirmPairing = useCoreAction("network.pair.confirm");

  const { data: pairingStatus, refetch: refetchStatus } = useCoreQuery(
    "network.pair.status",
    null,
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

  // Handle AwaitingUserConfirmation state
  useEffect(() => {
    if (
      currentSession?.state &&
      typeof currentSession.state === "object" &&
      "AwaitingUserConfirmation" in currentSession.state
    ) {
      const state = currentSession.state.AwaitingUserConfirmation;
      setConfirmationRequest({
        sessionId: currentSession.id,
        deviceName: currentSession.remote_device_name || "Unknown Device",
        deviceOs: currentSession.remote_device_os || "Unknown OS",
        confirmationCode: state.confirmation_code,
        expiresAt: new Date(state.expires_at),
      });
    } else if (
      currentSession?.confirmation_code &&
      currentSession?.confirmation_expires_at
    ) {
      // Fallback to top-level confirmation fields
      setConfirmationRequest({
        sessionId: currentSession.id,
        deviceName: currentSession.remote_device_name || "Unknown Device",
        deviceOs: currentSession.remote_device_os || "Unknown OS",
        confirmationCode: currentSession.confirmation_code,
        expiresAt: new Date(currentSession.confirmation_expires_at),
      });
    } else {
      setConfirmationRequest(null);
      setConfirmationInput("");
    }
  }, [currentSession]);

  // Countdown timer for confirmation
  useEffect(() => {
    if (!confirmationRequest) return;

    const updateTimer = () => {
      const now = new Date();
      const remaining = Math.max(
        0,
        Math.floor(
          (confirmationRequest.expiresAt.getTime() - now.getTime()) / 1000,
        ),
      );
      setTimeRemaining(remaining);
    };

    updateTimer();
    const interval = setInterval(updateTimer, 1000);
    return () => clearInterval(interval);
  }, [confirmationRequest]);

  const handleConfirmPairing = useCallback(
    (accepted: boolean) => {
      if (!confirmationRequest) return;
      confirmPairing.mutate({
        session_id: confirmationRequest.sessionId,
        accepted,
      });
      if (!accepted) {
        setConfirmationRequest(null);
        setConfirmationInput("");
      }
    },
    [confirmationRequest, confirmPairing],
  );

  const isCodeMatching =
    confirmationInput === confirmationRequest?.confirmationCode;

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
    confirmPairing.reset();
    setJoinCode("");
    setJoinNodeId("");
    setShowScanner(false);
    setConfirmationRequest(null);
    setConfirmationInput("");
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
          "[PairingPanel] QR scanned, auto-joining with full QR data",
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
        data,
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
          "Camera access is required to scan QR codes",
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
      visible={isOpen}
      animationType="slide"
      transparent
      onRequestClose={handleClose}
    >
      <View className="flex-1 bg-black/50">
        <View
          className="flex-1 bg-app-box rounded-t-3xl overflow-hidden"
          style={{ marginTop: insets.top + 40 }}
        >
          {/* Header */}
          <View className="px-6 py-4 border-b border-app-line">
            <View className="flex-row items-center justify-between">
              <View>
                <Text className="text-lg font-semibold text-ink">
                  Device Pairing
                </Text>
                <Text className="text-xs text-ink-dull mt-0.5">
                  Connect another device to share files
                </Text>
              </View>
              <Pressable
                onPress={handleClose}
                className="p-2 active:bg-app-hover rounded-lg"
              >
                <Text className="text-ink-dull text-xl">✕</Text>
              </Pressable>
            </View>
          </View>

          {/* Mode Tabs */}
          <View className="flex-row border-b border-app-line">
            <Pressable
              onPress={() => setMode("generate")}
              className={`flex-1 px-6 py-3 ${
                mode === "generate" ? "border-b-2 border-accent" : ""
              }`}
            >
              <Text
                className={`text-sm font-medium text-center ${
                  mode === "generate" ? "text-accent" : "text-ink-dull"
                }`}
              >
                Generate Code
              </Text>
            </Pressable>
            <Pressable
              onPress={() => setMode("join")}
              className={`flex-1 px-6 py-3 ${
                mode === "join" ? "border-b-2 border-accent" : ""
              }`}
            >
              <Text
                className={`text-sm font-medium text-center ${
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
            {/* User Confirmation Dialog */}
            {confirmationRequest && (
              <ConfirmationMode
                request={confirmationRequest}
                confirmationInput={confirmationInput}
                setConfirmationInput={setConfirmationInput}
                timeRemaining={timeRemaining}
                isCodeMatching={isCodeMatching}
                onConfirm={handleConfirmPairing}
                isPending={confirmPairing.isPending}
              />
            )}

            {/* Normal pairing modes - only show if not awaiting confirmation */}
            {!confirmationRequest && mode === "generate" ? (
              <GenerateMode
                generatePairing={generatePairing}
                currentSession={currentSession}
                onGenerate={handleGenerate}
                onCancel={handleCancel}
                onCopyCode={copyCode}
              />
            ) : !confirmationRequest && showScanner ? (
              <ScannerMode
                onScan={handleQRScan}
                onClose={() => setShowScanner(false)}
              />
            ) : !confirmationRequest ? (
              <JoinMode
                joinCode={joinCode}
                setJoinCode={setJoinCode}
                joinNodeId={joinNodeId}
                setJoinNodeId={setJoinNodeId}
                joinPairing={joinPairing}
                currentSession={currentSession}
                onJoin={handleJoin}
                onCancel={handleCancel}
                onOpenScanner={openScanner}
              />
            )}

            {/* Success State */}
            {isCompleted && (
              <View className="flex-row items-center gap-3 p-4 bg-accent/10 border border-accent/30 rounded-lg mt-6">
                <Text className="text-accent text-xl">✓</Text>
                <View className="flex-1">
                  <Text className="text-sm font-medium text-accent">
                    Pairing successful!
                  </Text>
                  <Text className="text-xs text-ink-dull mt-0.5">
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

function ConfirmationMode({
  request,
  confirmationInput,
  setConfirmationInput,
  timeRemaining,
  isCodeMatching,
  onConfirm,
  isPending,
}: {
  request: ConfirmationRequest;
  confirmationInput: string;
  setConfirmationInput: (value: string) => void;
  timeRemaining: number;
  isCodeMatching: boolean;
  onConfirm: (accepted: boolean) => void;
  isPending: boolean;
}) {
  return (
    <View className="gap-6">
      {/* Security Warning */}
      <View className="flex-row gap-3 p-4 bg-amber-500/10 rounded-lg border border-amber-500/30">
        <Text className="text-amber-500 text-xl">🛡️</Text>
        <View className="flex-1">
          <Text className="text-sm font-medium text-amber-500">
            Pairing Request
          </Text>
          <Text className="text-xs text-ink-dull mt-1">
            A device wants to pair with you. Verify the code matches before
            accepting.
          </Text>
        </View>
      </View>

      {/* Device Info */}
      <View className="p-4 bg-sidebar-box/40 rounded-lg border border-sidebar-line">
        <View className="flex-row items-center gap-3">
          <View className="w-12 h-12 rounded-full bg-accent/10 items-center justify-center">
            <Text className="text-accent text-2xl">📱</Text>
          </View>
          <View className="flex-1">
            <Text className="text-sm font-medium text-ink">
              {request.deviceName}
            </Text>
            <Text className="text-xs text-ink-dull">{request.deviceOs}</Text>
          </View>
        </View>
      </View>

      {/* Confirmation Code Display */}
      <View className="items-center gap-3">
        <Text className="text-sm text-ink-dull">Enter this code to confirm:</Text>
        <View className="px-8 py-4 bg-accent/10 border border-accent/30 rounded-xl">
          <Text className="text-4xl font-bold text-accent tracking-widest font-mono">
            {request.confirmationCode}
          </Text>
        </View>
      </View>

      {/* Code Input */}
      <View>
        <Text className="text-xs font-medium text-ink-dull uppercase tracking-wider mb-2">
          Enter Confirmation Code
        </Text>
        <TextInput
          value={confirmationInput}
          onChangeText={(text) =>
            setConfirmationInput(text.replace(/[^0-9]/g, "").slice(0, 2))
          }
          placeholder="00"
          placeholderTextColor="hsl(235, 10%, 55%)"
          maxLength={2}
          keyboardType="number-pad"
          editable={!isPending}
          className={`px-4 py-3 text-center text-2xl font-mono font-bold bg-sidebar-box border rounded-lg ${
            isCodeMatching
              ? "border-accent/50 text-accent"
              : confirmationInput.length === 2
                ? "border-red-500/50 text-red-500"
                : "border-sidebar-line text-ink"
          }`}
        />
        {confirmationInput.length === 2 && !isCodeMatching && (
          <Text className="text-xs text-red-500 mt-2 text-center">
            Code does not match
          </Text>
        )}
      </View>

      {/* Timer */}
      <View className="flex-row items-center justify-center gap-2">
        <Text className="text-sm text-ink-dull">⏱️</Text>
        <Text className="text-sm text-ink-dull">
          Expires in {Math.floor(timeRemaining / 60)}:
          {(timeRemaining % 60).toString().padStart(2, "0")}
        </Text>
      </View>

      {/* Action Buttons */}
      <View className="flex-row gap-3">
        <Pressable
          onPress={() => onConfirm(false)}
          disabled={isPending}
          className="flex-1 px-4 py-2.5 bg-app-box border border-app-line rounded-lg active:bg-app-hover"
        >
          <Text className="text-sm font-medium text-ink-dull text-center">
            Reject
          </Text>
        </Pressable>
        <Pressable
          onPress={() => onConfirm(true)}
          disabled={!isCodeMatching || isPending}
          className={`flex-1 flex-row items-center justify-center gap-2 px-4 py-2.5 rounded-lg ${
            isCodeMatching && !isPending
              ? "bg-accent active:bg-accent/90"
              : "bg-accent/50"
          }`}
        >
          {isPending && <ActivityIndicator size="small" color="white" />}
          <Text className="text-white font-medium">
            {isPending ? "Confirming..." : "Accept"}
          </Text>
        </Pressable>
      </View>
    </View>
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
      {!hasCode ? (
        <>
          {/* Info */}
          <View className="flex-row gap-3 p-4 bg-sidebar-box/40 rounded-lg border border-sidebar-line">
            <View className="w-10 h-10 rounded-full bg-accent/10 items-center justify-center">
              <Text className="text-accent text-xl">◊</Text>
            </View>
            <View className="flex-1">
              <Text className="text-sm font-medium text-ink">How it works</Text>
              <Text className="text-xs text-ink-dull mt-1">
                Generate a secure code to share with another device. They'll
                scan or enter the code to connect.
              </Text>
            </View>
          </View>

          {/* Generate Button */}
          <Pressable
            onPress={onGenerate}
            disabled={isLoading}
            className={`flex-row items-center justify-center gap-2 px-4 py-3 rounded-lg ${
              isLoading ? "bg-accent/50" : "bg-accent active:bg-accent/90"
            }`}
          >
            {isLoading && <ActivityIndicator size="small" color="white" />}
            <Text className="text-white font-medium">
              {isLoading ? "Generating..." : "Generate Pairing Code"}
            </Text>
          </Pressable>
        </>
      ) : (
        <>
          {/* QR Code */}
          <View className="items-center gap-3">
            <Text className="text-sm text-ink-dull">
              Scan with mobile device
            </Text>
            <View className="p-4 bg-white rounded-lg">
              <QRCode value={generatePairing.data.qr_json} size={200} />
            </View>
          </View>

          {/* Word Code */}
          <View>
            <Text className="text-xs font-medium text-ink-dull uppercase tracking-wider mb-2">
              Or type manually:
            </Text>
            <View className="p-4 bg-sidebar-box border border-sidebar-line rounded-lg">
              <Text className="font-mono text-sm text-ink">
                {generatePairing.data.code}
              </Text>
            </View>
            <Pressable
              onPress={onCopyCode}
              className="mt-2 p-2 bg-app-box border border-app-line rounded-lg active:bg-app-hover"
            >
              <Text className="text-sm text-ink-dull text-center">
                Copy Code
              </Text>
            </Pressable>
          </View>

          {/* Status */}
          {state && (
            <View className="flex-row items-center gap-2 p-3 bg-app-box/40 rounded-lg border border-app-line">
              <View className="w-2 h-2 rounded-full bg-accent" />
              <Text className="text-sm text-ink-dull">
                {state === "Broadcasting" || state === "WaitingForConnection"
                  ? "Waiting for device to connect..."
                  : state === "Authenticating"
                    ? "Authenticating device..."
                    : state === "ExchangingKeys"
                      ? "Exchanging encryption keys..."
                      : typeof state === "object" &&
                          "AwaitingUserConfirmation" in state
                        ? "Awaiting user confirmation..."
                        : "Ready to pair"}
              </Text>
            </View>
          )}

          {/* Cancel Button */}
          <Pressable
            onPress={onCancel}
            className="px-4 py-2.5 bg-app-box border border-app-line rounded-lg active:bg-app-hover"
          >
            <Text className="text-sm font-medium text-ink-dull text-center">
              Cancel
            </Text>
          </Pressable>
        </>
      )}

      {/* Error */}
      {generatePairing.isError && (
        <View className="flex-row gap-2 p-3 bg-red-500/10 border border-red-500/30 rounded-lg">
          <Text className="text-red-500 text-xl">⚠</Text>
          <View className="flex-1">
            <Text className="text-sm font-medium text-red-500">
              Failed to generate code
            </Text>
            <Text className="text-xs text-ink-dull mt-0.5">
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
      <View className="flex-row gap-3 p-4 bg-sidebar-box/40 rounded-lg border border-sidebar-line">
        <View className="w-10 h-10 rounded-full bg-accent/10 items-center justify-center">
          <Text className="text-accent text-xl">◊</Text>
        </View>
        <View className="flex-1">
          <Text className="text-sm font-medium text-ink">
            Enter pairing code
          </Text>
          <Text className="text-xs text-ink-dull mt-1">
            Scan the QR code or enter the 12-word code from the other device.
          </Text>
        </View>
      </View>

      {/* QR Scanner Button */}
      <Pressable
        onPress={onOpenScanner}
        disabled={isLoading || !!state}
        className={`flex-row items-center justify-center gap-2 px-4 py-3 rounded-lg border-2 border-dashed ${
          isLoading || state
            ? "border-app-line bg-app-box/50"
            : "border-accent bg-accent/10 active:bg-accent/20"
        }`}
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
        <Text className="text-xs font-medium text-ink-dull uppercase tracking-wider mb-2">
          Or Enter Code Manually
        </Text>
        <TextInput
          value={joinCode}
          onChangeText={setJoinCode}
          placeholder="brave-lion-sunset-river-eagle-mountain..."
          placeholderTextColor="hsl(235, 10%, 55%)"
          editable={!isLoading && !state}
          multiline
          numberOfLines={3}
          className="px-4 py-3 bg-sidebar-box border border-sidebar-line rounded-lg text-sm text-ink"
          style={{ textAlignVertical: "top" }}
        />
        <Text className="text-xs text-ink-dull mt-2">
          Paste the full code or type the 12 words separated by hyphens
        </Text>
      </View>

      {/* Node ID Input */}
      <View>
        <Text className="text-xs font-medium text-ink-dull uppercase tracking-wider mb-2">
          Node ID <Text className="text-ink-faint">(optional)</Text>
        </Text>
        <TextInput
          value={joinNodeId}
          onChangeText={setJoinNodeId}
          placeholder="For cross-network pairing"
          placeholderTextColor="hsl(235, 10%, 55%)"
          editable={!isLoading && !state}
          className="px-4 py-2.5 bg-sidebar-box border border-sidebar-line rounded-lg text-xs text-ink font-mono"
        />
      </View>

      {/* Status */}
      {state && (
        <View className="flex-row items-center gap-2 p-3 bg-app-box/40 rounded-lg border border-app-line">
          <View className="w-2 h-2 rounded-full bg-accent" />
          <Text className="text-sm text-ink-dull">
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
            onPress={onCancel}
            className="flex-1 px-4 py-2.5 bg-app-box border border-app-line rounded-lg active:bg-app-hover"
          >
            <Text className="text-sm font-medium text-ink-dull text-center">
              Cancel
            </Text>
          </Pressable>
        ) : (
          <>
            <Pressable
              onPress={onCancel}
              className="px-6 py-2.5 bg-app-box border border-app-line rounded-lg active:bg-app-hover"
            >
              <Text className="text-sm font-medium text-ink-dull text-center">
                Clear
              </Text>
            </Pressable>
            <Pressable
              onPress={onJoin}
              disabled={isLoading || !joinCode.trim()}
              className={`flex-1 flex-row items-center justify-center gap-2 px-4 py-2.5 rounded-lg ${
                isLoading || !joinCode.trim()
                  ? "bg-accent/50"
                  : "bg-accent active:bg-accent/90"
              }`}
            >
              {isLoading && <ActivityIndicator size="small" color="white" />}
              <Text className="text-white font-medium">
                {isLoading ? "Joining..." : "Join"}
              </Text>
            </Pressable>
          </>
        )}
      </View>

      {/* Error */}
      {joinPairing.isError && (
        <View className="flex-row gap-2 p-3 bg-red-500/10 border border-red-500/30 rounded-lg">
          <Text className="text-red-500 text-xl">⚠</Text>
          <View className="flex-1">
            <Text className="text-sm font-medium text-red-500">
              Failed to join
            </Text>
            <Text className="text-xs text-ink-dull mt-0.5">
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
    <View className="flex-1 -mx-6 -my-6">
      <CameraView
        style={{ flex: 1 }}
        facing="back"
        barcodeScannerSettings={{
          barcodeTypes: ["qr"],
        }}
        onBarcodeScanned={scanned ? undefined : handleBarCodeScanned}
      >
        <View className="flex-1 items-center justify-center">
          <View className="w-64 h-64 border-2 border-accent rounded-lg" />
          <Text className="text-white text-center mt-4 px-6">
            {scanned ? "Scanned! Processing..." : "Point camera at QR code"}
          </Text>
        </View>
      </CameraView>

      <Pressable
        onPress={onClose}
        className="absolute top-4 right-4 p-3 bg-black/50 rounded-full"
      >
        <Text className="text-white text-xl">✕</Text>
      </Pressable>
    </View>
  );
}
