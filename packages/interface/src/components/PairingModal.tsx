import { useState, useEffect, useRef, useCallback } from "react";
import {
  QrCode,
  X,
  ArrowsClockwise,
  Check,
  Warning,
  DeviceMobile,
  Copy,
  CaretDown,
  ShieldCheck,
  Clock,
} from "@phosphor-icons/react";
import { motion, AnimatePresence } from "framer-motion";
import clsx from "clsx";
import QRCode from "qrcode";
import { useCoreMutation, useCoreQuery } from "../context";
import { sounds } from "@sd/assets/sounds";

interface PairingModalProps {
  isOpen: boolean;
  onClose: () => void;
  mode?: "generate" | "join";
}

interface ConfirmationRequest {
  sessionId: string;
  deviceName: string;
  deviceOs: string;
  confirmationCode: string;
  expiresAt: Date;
}

export function PairingModal({ isOpen, onClose, mode: initialMode = "generate" }: PairingModalProps) {
  const [mode, setMode] = useState<"generate" | "join">(initialMode);
  const [joinCode, setJoinCode] = useState("");
  const [joinNodeId, setJoinNodeId] = useState("");
  const [confirmationInput, setConfirmationInput] = useState("");
  const [confirmationRequest, setConfirmationRequest] = useState<ConfirmationRequest | null>(null);
  const [timeRemaining, setTimeRemaining] = useState<number>(60);

  const generatePairing = useCoreMutation("network.pair.generate");
  const joinPairing = useCoreMutation("network.pair.join");
  const cancelPairing = useCoreMutation("network.pair.cancel");
  const confirmPairing = useCoreMutation("network.pair.confirm");

  const { data: pairingStatus, refetch: refetchStatus } = useCoreQuery({
    type: "network.pair.status",
    input: null,
  });

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
    if (currentSession?.state && typeof currentSession.state === 'object' && 'AwaitingUserConfirmation' in currentSession.state) {
      const state = currentSession.state.AwaitingUserConfirmation;
      setConfirmationRequest({
        sessionId: currentSession.id,
        deviceName: currentSession.remote_device_name || "Unknown Device",
        deviceOs: currentSession.remote_device_os || "Unknown OS",
        confirmationCode: state.confirmation_code,
        expiresAt: new Date(state.expires_at),
      });
    } else if (currentSession?.confirmation_code && currentSession?.confirmation_expires_at) {
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
      const remaining = Math.max(0, Math.floor((confirmationRequest.expiresAt.getTime() - now.getTime()) / 1000));
      setTimeRemaining(remaining);
    };

    updateTimer();
    const interval = setInterval(updateTimer, 1000);
    return () => clearInterval(interval);
  }, [confirmationRequest]);

  const handleConfirmPairing = useCallback((accepted: boolean) => {
    if (!confirmationRequest) return;
    confirmPairing.mutate({
      session_id: confirmationRequest.sessionId,
      accepted,
    });
    if (!accepted) {
      setConfirmationRequest(null);
      setConfirmationInput("");
    }
  }, [confirmationRequest, confirmPairing]);

  const isCodeMatching = confirmationInput === confirmationRequest?.confirmationCode;

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
    setConfirmationRequest(null);
    setConfirmationInput("");
  };

  const handleClose = () => {
    handleCancel();
    onClose();
  };

  const copyCode = () => {
    if (generatePairing.data?.code) {
      navigator.clipboard.writeText(generatePairing.data.code);
    }
  };

  // Check if pairing completed
  const isCompleted = currentSession?.state === "Completed" || joinPairing.isSuccess;

  useEffect(() => {
    if (isCompleted) {
      sounds.pairing();
      const timer = setTimeout(() => {
        handleClose();
      }, 2000);
      return () => clearTimeout(timer);
    }
  }, [isCompleted]);

  if (!isOpen) return null;

  return (
    <AnimatePresence>
      <div className="fixed inset-0 z-[100] flex items-center justify-center">
        {/* Backdrop */}
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="absolute inset-0 bg-black/50 backdrop-blur-sm"
          onClick={handleClose}
        />

        {/* Modal */}
        <motion.div
          initial={{ opacity: 0, scale: 0.95 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 0.95 }}
          transition={{ duration: 0.2 }}
          className="relative w-full max-w-xl bg-app-box border border-app-line rounded-xl shadow-2xl overflow-hidden"
        >
          {/* Header */}
          <div className="flex items-center justify-between px-6 py-4 border-b border-app-line">
            <div className="flex items-center gap-3">
              <DeviceMobile className="size-6 text-accent" weight="bold" />
              <div>
                <h2 className="text-lg font-semibold text-ink">Device Pairing</h2>
                <p className="text-xs text-ink-dull">Connect another device to share files</p>
              </div>
            </div>
            <button
              onClick={handleClose}
              className="p-1.5 hover:bg-app-hover rounded-lg transition-colors"
            >
              <X className="size-5 text-ink-dull" weight="bold" />
            </button>
          </div>

          {/* Mode Tabs */}
          <div className="flex border-b border-app-line">
            <button
              onClick={() => setMode("generate")}
              className={clsx(
                "flex-1 px-6 py-3 text-sm font-medium transition-colors",
                mode === "generate"
                  ? "text-accent border-b-2 border-accent"
                  : "text-ink-dull hover:text-ink"
              )}
            >
              Generate Code
            </button>
            <button
              onClick={() => setMode("join")}
              className={clsx(
                "flex-1 px-6 py-3 text-sm font-medium transition-colors",
                mode === "join"
                  ? "text-accent border-b-2 border-accent"
                  : "text-ink-dull hover:text-ink"
              )}
            >
              Join with Code
            </button>
          </div>

          {/* Content */}
          <div className="p-6 space-y-6">
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
            ) : !confirmationRequest && (
              <JoinMode
                joinCode={joinCode}
                setJoinCode={setJoinCode}
                joinNodeId={joinNodeId}
                setJoinNodeId={setJoinNodeId}
                joinPairing={joinPairing}
                currentSession={currentSession}
                onJoin={handleJoin}
                onCancel={handleCancel}
              />
            )}

            {/* Success State */}
            {isCompleted && (
              <motion.div
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                className="flex items-center gap-3 p-4 bg-accent/10 border border-accent/30 rounded-lg"
              >
                <Check className="size-5 text-accent" weight="bold" />
                <div className="flex-1">
                  <p className="text-sm font-medium text-accent">Pairing successful!</p>
                  <p className="text-xs text-ink-dull mt-0.5">
                    {joinPairing.data ? `Connected to ${joinPairing.data.device_name}` : "Device paired"}
                  </p>
                </div>
              </motion.div>
            )}
          </div>
        </motion.div>
      </div>
    </AnimatePresence>
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
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  return (
    <div className="space-y-6">
      {/* Security Warning */}
      <div className="flex items-start gap-3 p-4 bg-amber-500/10 border border-amber-500/30 rounded-lg">
        <ShieldCheck className="size-6 text-amber-500 shrink-0 mt-0.5" weight="bold" />
        <div className="flex-1">
          <h3 className="text-sm font-medium text-amber-500">Pairing Request</h3>
          <p className="text-xs text-ink-dull mt-1">
            A device wants to pair with you. Verify the code matches before accepting.
          </p>
        </div>
      </div>

      {/* Device Info */}
      <div className="p-4 bg-sidebar-box/40 rounded-lg border border-sidebar-line">
        <div className="flex items-center gap-3">
          <div className="size-12 rounded-full bg-accent/10 flex items-center justify-center">
            <DeviceMobile className="size-6 text-accent" weight="bold" />
          </div>
          <div className="flex-1">
            <p className="text-sm font-medium text-ink">{request.deviceName}</p>
            <p className="text-xs text-ink-dull">{request.deviceOs}</p>
          </div>
        </div>
      </div>

      {/* Confirmation Code Display */}
      <div className="text-center space-y-3">
        <p className="text-sm text-ink-dull">Enter this code to confirm:</p>
        <div className="inline-flex items-center justify-center px-8 py-4 bg-accent/10 border border-accent/30 rounded-xl">
          <span className="text-4xl font-bold text-accent tracking-[0.5em] font-mono">
            {request.confirmationCode}
          </span>
        </div>
      </div>

      {/* Code Input */}
      <div>
        <label className="text-xs font-medium text-ink-dull uppercase tracking-wider mb-2 block">
          Enter Confirmation Code
        </label>
        <input
          ref={inputRef}
          type="text"
          value={confirmationInput}
          onChange={(e) => setConfirmationInput(e.target.value.replace(/[^0-9]/g, '').slice(0, 2))}
          placeholder="00"
          maxLength={2}
          disabled={isPending}
          className={clsx(
            "w-full px-4 py-3 text-center text-2xl font-mono font-bold bg-sidebar-box border rounded-lg transition-colors focus:outline-none focus:ring-2",
            isCodeMatching
              ? "border-accent/50 focus:ring-accent/50 text-accent"
              : confirmationInput.length === 2
              ? "border-red-500/50 focus:ring-red-500/50 text-red-500"
              : "border-sidebar-line focus:ring-accent/50 text-ink"
          )}
        />
        {confirmationInput.length === 2 && !isCodeMatching && (
          <p className="text-xs text-red-500 mt-2 text-center">Code does not match</p>
        )}
      </div>

      {/* Timer */}
      <div className="flex items-center justify-center gap-2 text-sm text-ink-dull">
        <Clock className="size-4" weight="bold" />
        <span>
          Expires in {Math.floor(timeRemaining / 60)}:{(timeRemaining % 60).toString().padStart(2, '0')}
        </span>
      </div>

      {/* Action Buttons */}
      <div className="flex gap-3">
        <button
          onClick={() => onConfirm(false)}
          disabled={isPending}
          className="flex-1 px-4 py-2.5 text-sm font-medium text-ink-dull hover:text-ink bg-app-box hover:bg-app-hover border border-app-line rounded-lg transition-colors disabled:opacity-50"
        >
          Reject
        </button>
        <button
          onClick={() => onConfirm(true)}
          disabled={!isCodeMatching || isPending}
          className={clsx(
            "flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg font-medium transition-colors",
            isCodeMatching && !isPending
              ? "bg-accent hover:bg-accent/90 text-white"
              : "bg-accent/50 text-white/70 cursor-not-allowed"
          )}
        >
          {isPending && <ArrowsClockwise className="size-5 animate-spin" weight="bold" />}
          {isPending ? "Confirming..." : "Accept"}
        </button>
      </div>
    </div>
  );
}

function GenerateMode({
  generatePairing,
  currentSession,
  onGenerate,
  onCancel,
  onCopyCode,
}: any) {
  const [showAdvanced, setShowAdvanced] = useState(false);
  const hasCode = generatePairing.data?.code;
  const isLoading = generatePairing.isPending;
  const state = currentSession?.state;

  const copyQRJson = () => {
    if (generatePairing.data?.qr_json) {
      navigator.clipboard.writeText(generatePairing.data.qr_json);
    }
  };

  return (
    <>
      {!hasCode ? (
        <>
          {/* Setup */}
          <div className="space-y-4">
            <div className="flex items-start gap-3 p-4 bg-sidebar-box/40 rounded-lg border border-sidebar-line">
              <div className="size-10 rounded-full bg-accent/10 flex items-center justify-center shrink-0">
                <QrCode className="size-5 text-accent" weight="bold" />
              </div>
              <div className="flex-1">
                <h3 className="text-sm font-medium text-ink">How it works</h3>
                <p className="text-xs text-ink-dull mt-1">
                  Generate a secure code to share with another device. They'll enter the code to establish a trusted connection.
                </p>
              </div>
            </div>
          </div>

          {/* Generate Button */}
          <button
            onClick={onGenerate}
            disabled={isLoading}
            className={clsx(
              "w-full flex items-center justify-center gap-2 px-4 py-3 rounded-lg font-medium transition-colors",
              isLoading
                ? "bg-accent/50 cursor-not-allowed"
                : "bg-accent hover:bg-accent/90 text-white"
            )}
          >
            {isLoading && <ArrowsClockwise className="size-5 animate-spin" weight="bold" />}
            {isLoading ? "Generating..." : "Generate Pairing Code"}
          </button>
        </>
      ) : (
        <>
          {/* Generated Code Display */}
          <div className="space-y-4">
            {/* QR Code */}
            <div className="flex flex-col items-center gap-3">
              <p className="text-sm text-ink-dull">Scan with mobile device</p>
              <div className="inline-block p-4 bg-white rounded-lg">
                <QRCodeCanvas data={generatePairing.data.qr_json} size={192} />
              </div>
            </div>

            {/* Word Code */}
            <div>
              <label className="text-xs font-medium text-ink-dull uppercase tracking-wider mb-2 block">
                Or type manually:
              </label>
              <div className="relative">
                <div className="p-4 bg-sidebar-box border border-sidebar-line rounded-lg font-mono text-sm text-ink break-all">
                  {generatePairing.data.code}
                </div>
                <button
                  onClick={onCopyCode}
                  className="absolute top-2 right-2 p-2 bg-app-box hover:bg-app-hover border border-app-line rounded-md transition-colors"
                  title="Copy code"
                >
                  <Copy className="size-4 text-ink-dull" weight="bold" />
                </button>
              </div>
            </div>

            {/* Node ID for cross-network pairing */}
            {generatePairing.data.node_id && (
              <div>
                <label className="text-xs font-medium text-ink-dull uppercase tracking-wider mb-2 block">
                  For cross-network pairing:
                </label>
                <div className="relative">
                  <div className="p-3 bg-sidebar-box border border-sidebar-line rounded-lg font-mono text-xs text-ink break-all">
                    {generatePairing.data.node_id}
                  </div>
                  <button
                    onClick={() => navigator.clipboard.writeText(generatePairing.data.node_id)}
                    className="absolute top-1.5 right-1.5 p-1.5 bg-app-box hover:bg-app-hover border border-app-line rounded-md transition-colors"
                    title="Copy Node ID"
                  >
                    <Copy className="size-3 text-ink-dull" weight="bold" />
                  </button>
                </div>
                <p className="text-xs text-ink-dull mt-1.5">
                  Share this Node ID if devices are on different networks
                </p>
              </div>
            )}

            {/* Status */}
            <div className="flex items-center gap-2 p-3 bg-app-box/40 rounded-lg border border-app-line">
              <div className="size-2 rounded-full bg-accent animate-pulse" />
              <span className="text-sm text-ink-dull">
                {state === "Broadcasting" || state === "WaitingForConnection"
                  ? "Waiting for device to connect..."
                  : state === "Authenticating"
                  ? "Authenticating device..."
                  : state === "ExchangingKeys"
                  ? "Exchanging encryption keys..."
                  : typeof state === 'object' && 'AwaitingUserConfirmation' in state
                  ? "Awaiting user confirmation..."
                  : "Ready to pair"}
              </span>
            </div>

            {/* Advanced Section */}
            <div className="border-t border-app-line pt-4">
              <button
                onClick={() => setShowAdvanced(!showAdvanced)}
                className="flex items-center gap-2 text-xs text-ink-dull hover:text-ink transition-colors mx-auto"
              >
                <span>Advanced</span>
                <CaretDown
                  className={clsx("size-3 transition-transform", showAdvanced && "rotate-180")}
                  weight="bold"
                />
              </button>

              <AnimatePresence>
                {showAdvanced && (
                  <motion.div
                    initial={{ height: 0, opacity: 0 }}
                    animate={{ height: "auto", opacity: 1 }}
                    exit={{ height: 0, opacity: 0 }}
                    transition={{ duration: 0.15 }}
                    className="overflow-hidden"
                  >
                    <div className="mt-3 space-y-2">
                      <div className="p-3 bg-sidebar-box/40 rounded-lg border border-sidebar-line">
                        <div className="flex items-center justify-between mb-1">
                          <span className="text-xs text-ink-dull">Session ID</span>
                          <button
                            onClick={() => navigator.clipboard.writeText(generatePairing.data.session_id)}
                            className="p-1 hover:bg-app-hover rounded transition-colors"
                          >
                            <Copy className="size-3 text-ink-dull" weight="bold" />
                          </button>
                        </div>
                        <code className="text-xs text-ink font-mono break-all">
                          {generatePairing.data.session_id}
                        </code>
                      </div>

                      <div className="p-3 bg-sidebar-box/40 rounded-lg border border-sidebar-line">
                        <div className="flex items-center justify-between mb-1">
                          <span className="text-xs text-ink-dull">QR JSON</span>
                          <button
                            onClick={copyQRJson}
                            className="p-1 hover:bg-app-hover rounded transition-colors"
                          >
                            <Copy className="size-3 text-ink-dull" weight="bold" />
                          </button>
                        </div>
                        <code className="text-xs text-ink font-mono break-all line-clamp-2">
                          {generatePairing.data.qr_json}
                        </code>
                      </div>

                      {generatePairing.data.expires_at && (
                        <p className="text-xs text-ink-dull text-center pt-2">
                          Expires at {new Date(generatePairing.data.expires_at).toLocaleTimeString()}
                        </p>
                      )}
                    </div>
                  </motion.div>
                )}
              </AnimatePresence>
            </div>
          </div>

          {/* Cancel Button */}
          <button
            onClick={onCancel}
            className="w-full px-4 py-2.5 text-sm font-medium text-ink-dull hover:text-ink bg-app-box hover:bg-app-hover border border-app-line rounded-lg transition-colors"
          >
            Cancel
          </button>
        </>
      )}

      {/* Error */}
      {generatePairing.isError && (
        <div className="flex items-start gap-2 p-3 bg-red-500/10 border border-red-500/30 rounded-lg">
          <Warning className="size-5 text-red-500 shrink-0 mt-0.5" weight="bold" />
          <div className="flex-1">
            <p className="text-sm font-medium text-red-500">Failed to generate code</p>
            <p className="text-xs text-ink-dull mt-0.5">{String(generatePairing.error)}</p>
          </div>
        </div>
      )}
    </>
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
}: any) {
  const isLoading = joinPairing.isPending;
  const state = currentSession?.state;

  return (
    <>
      <div className="space-y-4">
        {/* Instructions */}
        <div className="flex items-start gap-3 p-4 bg-sidebar-box/40 rounded-lg border border-sidebar-line">
          <div className="size-10 rounded-full bg-accent/10 flex items-center justify-center shrink-0">
            <QrCode className="size-5 text-accent" weight="bold" />
          </div>
          <div className="flex-1">
            <h3 className="text-sm font-medium text-ink">Enter pairing code</h3>
            <p className="text-xs text-ink-dull mt-1">
              Enter the 12-word code from the other device to establish a secure connection.
            </p>
          </div>
        </div>

        {/* Code Input */}
        <div>
          <label className="text-xs font-medium text-ink-dull uppercase tracking-wider mb-2 block">
            Pairing Code
          </label>
          <textarea
            value={joinCode}
            onChange={(e) => setJoinCode(e.target.value)}
            placeholder="brave-lion-sunset-river-eagle-mountain-forest-ocean-thunder-crystal-diamond-phoenix"
            rows={3}
            disabled={isLoading || !!state}
            className="w-full px-4 py-3 bg-sidebar-box border border-sidebar-line rounded-lg text-sm text-ink placeholder:text-ink-faint resize-none focus:outline-none focus:ring-2 focus:ring-accent/50 disabled:opacity-50"
          />
          <p className="text-xs text-ink-dull mt-2">
            Paste the full code or type the 12 words separated by hyphens
          </p>
        </div>

        {/* Node ID Input (optional, for cross-network) */}
        <div>
          <label className="text-xs font-medium text-ink-dull uppercase tracking-wider mb-2 block">
            Node ID <span className="text-ink-faint">(optional)</span>
          </label>
          <input
            type="text"
            value={joinNodeId}
            onChange={(e) => setJoinNodeId(e.target.value)}
            placeholder="Enter Node ID for cross-network pairing"
            disabled={isLoading || !!state}
            className="w-full px-4 py-2.5 bg-sidebar-box border border-sidebar-line rounded-lg text-xs text-ink font-mono placeholder:text-ink-faint focus:outline-none focus:ring-2 focus:ring-accent/50 disabled:opacity-50"
          />
          <p className="text-xs text-ink-dull mt-1.5">
            Required if devices are on different networks
          </p>
        </div>

        {/* Status */}
        {state && (
          <div className="flex items-center gap-2 p-3 bg-app-box/40 rounded-lg border border-app-line">
            <div className="size-2 rounded-full bg-accent animate-pulse" />
            <span className="text-sm text-ink-dull">
              {state === "Scanning" || state === "Connecting"
                ? "Finding device..."
                : state === "Authenticating"
                ? "Authenticating..."
                : state === "ExchangingKeys"
                ? "Exchanging keys..."
                : state === "EstablishingSession"
                ? "Establishing secure session..."
                : "Processing..."}
            </span>
          </div>
        )}
      </div>

      {/* Action Buttons */}
      <div className="flex gap-3">
        {state ? (
          <button
            onClick={onCancel}
            className="flex-1 px-4 py-2.5 text-sm font-medium text-ink-dull hover:text-ink bg-app-box hover:bg-app-hover border border-app-line rounded-lg transition-colors"
          >
            Cancel
          </button>
        ) : (
          <>
            <button
              onClick={onCancel}
              className="px-6 py-2.5 text-sm font-medium text-ink-dull hover:text-ink bg-app-box hover:bg-app-hover border border-app-line rounded-lg transition-colors"
            >
              Clear
            </button>
            <button
              onClick={onJoin}
              disabled={isLoading || !joinCode.trim()}
              className={clsx(
                "flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg font-medium transition-colors",
                isLoading || !joinCode.trim()
                  ? "bg-accent/50 text-white/70 cursor-not-allowed"
                  : "bg-accent hover:bg-accent/90 text-white"
              )}
            >
              {isLoading && <ArrowsClockwise className="size-5 animate-spin" weight="bold" />}
              {isLoading ? "Joining..." : "Join"}
            </button>
          </>
        )}
      </div>

      {/* Error */}
      {joinPairing.isError && (
        <div className="flex items-start gap-2 p-3 bg-red-500/10 border border-red-500/30 rounded-lg">
          <Warning className="size-5 text-red-500 shrink-0 mt-0.5" weight="bold" />
          <div className="flex-1">
            <p className="text-sm font-medium text-red-500">Failed to join</p>
            <p className="text-xs text-ink-dull mt-0.5">{String(joinPairing.error)}</p>
          </div>
        </div>
      )}
    </>
  );
}

// QR Code Canvas Component (avoids React version conflicts)
function QRCodeCanvas({ data, size }: { data: string; size: number }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    if (canvasRef.current && data) {
      QRCode.toCanvas(canvasRef.current, data, {
        width: size,
        margin: 1,
        errorCorrectionLevel: "M",
      }).catch((err) => {
        console.error("Failed to generate QR code:", err);
      });
    }
  }, [data, size]);

  return <canvas ref={canvasRef} />;
}
