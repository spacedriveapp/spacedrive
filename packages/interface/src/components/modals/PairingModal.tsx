import {
  ArrowsClockwise,
  CaretDown,
  Check,
  Copy,
  DeviceMobile,
  QrCode,
  Warning,
  X,
} from "@phosphor-icons/react";
import { sounds } from "@sd/assets/sounds";
import clsx from "clsx";
import { AnimatePresence, motion } from "framer-motion";
import QRCode from "qrcode";
import { useEffect, useRef, useState } from "react";
import {
  useCoreMutation,
  useCoreQuery,
} from "../../contexts/SpacedriveContext";

interface PairingModalProps {
  isOpen: boolean;
  onClose: () => void;
  mode?: "generate" | "join";
}

export function PairingModal({
  isOpen,
  onClose,
  mode: initialMode = "generate",
}: PairingModalProps) {
  const [mode, setMode] = useState<"generate" | "join">(initialMode);
  const [joinCode, setJoinCode] = useState("");
  const [joinNodeId, setJoinNodeId] = useState("");

  const generatePairing = useCoreMutation("network.pair.generate");
  const joinPairing = useCoreMutation("network.pair.join");
  const cancelPairing = useCoreMutation("network.pair.cancel");

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
  const isCompleted =
    currentSession?.state === "Completed" || joinPairing.isSuccess;

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
          animate={{ opacity: 1 }}
          className="absolute inset-0 bg-black/50 backdrop-blur-sm"
          exit={{ opacity: 0 }}
          initial={{ opacity: 0 }}
          onClick={handleClose}
        />

        {/* Modal */}
        <motion.div
          animate={{ opacity: 1, scale: 1 }}
          className="relative w-full max-w-xl overflow-hidden rounded-xl border border-app-line bg-app-box shadow-2xl"
          exit={{ opacity: 0, scale: 0.95 }}
          initial={{ opacity: 0, scale: 0.95 }}
          transition={{ duration: 0.2 }}
        >
          {/* Header */}
          <div className="flex items-center justify-between border-app-line border-b px-6 py-4">
            <div className="flex items-center gap-3">
              <DeviceMobile className="size-6 text-accent" weight="bold" />
              <div>
                <h2 className="font-semibold text-ink text-lg">
                  Device Pairing
                </h2>
                <p className="text-ink-dull text-xs">
                  Connect another device to share files
                </p>
              </div>
            </div>
            <button
              className="rounded-lg p-1.5 transition-colors hover:bg-app-hover"
              onClick={handleClose}
            >
              <X className="size-5 text-ink-dull" weight="bold" />
            </button>
          </div>

          {/* Mode Tabs */}
          <div className="flex border-app-line border-b">
            <button
              className={clsx(
                "flex-1 px-6 py-3 font-medium text-sm transition-colors",
                mode === "generate"
                  ? "border-accent border-b-2 text-accent"
                  : "text-ink-dull hover:text-ink"
              )}
              onClick={() => setMode("generate")}
            >
              Generate Code
            </button>
            <button
              className={clsx(
                "flex-1 px-6 py-3 font-medium text-sm transition-colors",
                mode === "join"
                  ? "border-accent border-b-2 text-accent"
                  : "text-ink-dull hover:text-ink"
              )}
              onClick={() => setMode("join")}
            >
              Join with Code
            </button>
          </div>

          {/* Content */}
          <div className="space-y-6 p-6">
            {mode === "generate" ? (
              <GenerateMode
                currentSession={currentSession}
                generatePairing={generatePairing}
                onCancel={handleCancel}
                onCopyCode={copyCode}
                onGenerate={handleGenerate}
              />
            ) : (
              <JoinMode
                currentSession={currentSession}
                joinCode={joinCode}
                joinNodeId={joinNodeId}
                joinPairing={joinPairing}
                onCancel={handleCancel}
                onJoin={handleJoin}
                setJoinCode={setJoinCode}
                setJoinNodeId={setJoinNodeId}
              />
            )}

            {/* Success State */}
            {isCompleted && (
              <motion.div
                animate={{ opacity: 1, y: 0 }}
                className="flex items-center gap-3 rounded-lg border border-accent/30 bg-accent/10 p-4"
                initial={{ opacity: 0, y: 10 }}
              >
                <Check className="size-5 text-accent" weight="bold" />
                <div className="flex-1">
                  <p className="font-medium text-accent text-sm">
                    Pairing successful!
                  </p>
                  <p className="mt-0.5 text-ink-dull text-xs">
                    {joinPairing.data
                      ? `Connected to ${joinPairing.data.device_name}`
                      : "Device paired"}
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
      {hasCode ? (
        <>
          {/* Generated Code Display */}
          <div className="space-y-4">
            {/* QR Code */}
            <div className="flex flex-col items-center gap-3">
              <p className="text-ink-dull text-sm">Scan with mobile device</p>
              <div className="inline-block rounded-lg bg-white p-4">
                <QRCodeCanvas data={generatePairing.data.qr_json} size={192} />
              </div>
            </div>

            {/* Word Code */}
            <div>
              <label className="mb-2 block font-medium text-ink-dull text-xs uppercase tracking-wider">
                Or type manually:
              </label>
              <div className="relative">
                <div className="break-all rounded-lg border border-sidebar-line bg-sidebar-box p-4 font-mono text-ink text-sm">
                  {generatePairing.data.code}
                </div>
                <button
                  className="absolute top-2 right-2 rounded-md border border-app-line bg-app-box p-2 transition-colors hover:bg-app-hover"
                  onClick={onCopyCode}
                  title="Copy code"
                >
                  <Copy className="size-4 text-ink-dull" weight="bold" />
                </button>
              </div>
            </div>

            {/* Node ID for cross-network pairing */}
            {generatePairing.data.node_id && (
              <div>
                <label className="mb-2 block font-medium text-ink-dull text-xs uppercase tracking-wider">
                  For cross-network pairing:
                </label>
                <div className="relative">
                  <div className="break-all rounded-lg border border-sidebar-line bg-sidebar-box p-3 font-mono text-ink text-xs">
                    {generatePairing.data.node_id}
                  </div>
                  <button
                    className="absolute top-1.5 right-1.5 rounded-md border border-app-line bg-app-box p-1.5 transition-colors hover:bg-app-hover"
                    onClick={() =>
                      navigator.clipboard.writeText(
                        generatePairing.data.node_id
                      )
                    }
                    title="Copy Node ID"
                  >
                    <Copy className="size-3 text-ink-dull" weight="bold" />
                  </button>
                </div>
                <p className="mt-1.5 text-ink-dull text-xs">
                  Share this Node ID if devices are on different networks
                </p>
              </div>
            )}

            {/* Status */}
            <div className="flex items-center gap-2 rounded-lg border border-app-line bg-app-box/40 p-3">
              <div className="size-2 animate-pulse rounded-full bg-accent" />
              <span className="text-ink-dull text-sm">
                {state === "Broadcasting" || state === "WaitingForConnection"
                  ? "Waiting for device to connect..."
                  : state === "Authenticating"
                    ? "Authenticating device..."
                    : state === "ExchangingKeys"
                      ? "Exchanging encryption keys..."
                      : "Ready to pair"}
              </span>
            </div>

            {/* Advanced Section */}
            <div className="border-app-line border-t pt-4">
              <button
                className="mx-auto flex items-center gap-2 text-ink-dull text-xs transition-colors hover:text-ink"
                onClick={() => setShowAdvanced(!showAdvanced)}
              >
                <span>Advanced</span>
                <CaretDown
                  className={clsx(
                    "size-3 transition-transform",
                    showAdvanced && "rotate-180"
                  )}
                  weight="bold"
                />
              </button>

              <AnimatePresence>
                {showAdvanced && (
                  <motion.div
                    animate={{ height: "auto", opacity: 1 }}
                    className="overflow-hidden"
                    exit={{ height: 0, opacity: 0 }}
                    initial={{ height: 0, opacity: 0 }}
                    transition={{ duration: 0.15 }}
                  >
                    <div className="mt-3 space-y-2">
                      <div className="rounded-lg border border-sidebar-line bg-sidebar-box/40 p-3">
                        <div className="mb-1 flex items-center justify-between">
                          <span className="text-ink-dull text-xs">
                            Session ID
                          </span>
                          <button
                            className="rounded p-1 transition-colors hover:bg-app-hover"
                            onClick={() =>
                              navigator.clipboard.writeText(
                                generatePairing.data.session_id
                              )
                            }
                          >
                            <Copy
                              className="size-3 text-ink-dull"
                              weight="bold"
                            />
                          </button>
                        </div>
                        <code className="break-all font-mono text-ink text-xs">
                          {generatePairing.data.session_id}
                        </code>
                      </div>

                      <div className="rounded-lg border border-sidebar-line bg-sidebar-box/40 p-3">
                        <div className="mb-1 flex items-center justify-between">
                          <span className="text-ink-dull text-xs">QR JSON</span>
                          <button
                            className="rounded p-1 transition-colors hover:bg-app-hover"
                            onClick={copyQRJson}
                          >
                            <Copy
                              className="size-3 text-ink-dull"
                              weight="bold"
                            />
                          </button>
                        </div>
                        <code className="line-clamp-2 break-all font-mono text-ink text-xs">
                          {generatePairing.data.qr_json}
                        </code>
                      </div>

                      {generatePairing.data.expires_at && (
                        <p className="pt-2 text-center text-ink-dull text-xs">
                          Expires at{" "}
                          {new Date(
                            generatePairing.data.expires_at
                          ).toLocaleTimeString()}
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
            className="w-full rounded-lg border border-app-line bg-app-box px-4 py-2.5 font-medium text-ink-dull text-sm transition-colors hover:bg-app-hover hover:text-ink"
            onClick={onCancel}
          >
            Cancel
          </button>
        </>
      ) : (
        <>
          {/* Setup */}
          <div className="space-y-4">
            <div className="flex items-start gap-3 rounded-lg border border-sidebar-line bg-sidebar-box/40 p-4">
              <div className="flex size-10 shrink-0 items-center justify-center rounded-full bg-accent/10">
                <QrCode className="size-5 text-accent" weight="bold" />
              </div>
              <div className="flex-1">
                <h3 className="font-medium text-ink text-sm">How it works</h3>
                <p className="mt-1 text-ink-dull text-xs">
                  Generate a secure code to share with another device. They'll
                  enter the code to establish a trusted connection.
                </p>
              </div>
            </div>
          </div>

          {/* Generate Button */}
          <button
            className={clsx(
              "flex w-full items-center justify-center gap-2 rounded-lg px-4 py-3 font-medium transition-colors",
              isLoading
                ? "cursor-not-allowed bg-accent/50"
                : "bg-accent text-white hover:bg-accent/90"
            )}
            disabled={isLoading}
            onClick={onGenerate}
          >
            {isLoading && (
              <ArrowsClockwise className="size-5 animate-spin" weight="bold" />
            )}
            {isLoading ? "Generating..." : "Generate Pairing Code"}
          </button>
        </>
      )}

      {/* Error */}
      {generatePairing.isError && (
        <div className="flex items-start gap-2 rounded-lg border border-red-500/30 bg-red-500/10 p-3">
          <Warning
            className="mt-0.5 size-5 shrink-0 text-red-500"
            weight="bold"
          />
          <div className="flex-1">
            <p className="font-medium text-red-500 text-sm">
              Failed to generate code
            </p>
            <p className="mt-0.5 text-ink-dull text-xs">
              {String(generatePairing.error)}
            </p>
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
        <div className="flex items-start gap-3 rounded-lg border border-sidebar-line bg-sidebar-box/40 p-4">
          <div className="flex size-10 shrink-0 items-center justify-center rounded-full bg-accent/10">
            <QrCode className="size-5 text-accent" weight="bold" />
          </div>
          <div className="flex-1">
            <h3 className="font-medium text-ink text-sm">Enter pairing code</h3>
            <p className="mt-1 text-ink-dull text-xs">
              Enter the 12-word code from the other device to establish a secure
              connection.
            </p>
          </div>
        </div>

        {/* Code Input */}
        <div>
          <label className="mb-2 block font-medium text-ink-dull text-xs uppercase tracking-wider">
            Pairing Code
          </label>
          <textarea
            className="w-full resize-none rounded-lg border border-sidebar-line bg-sidebar-box px-4 py-3 text-ink text-sm placeholder:text-ink-faint focus:outline-none focus:ring-2 focus:ring-accent/50 disabled:opacity-50"
            disabled={isLoading || !!state}
            onChange={(e) => setJoinCode(e.target.value)}
            placeholder="brave-lion-sunset-river-eagle-mountain-forest-ocean-thunder-crystal-diamond-phoenix"
            rows={3}
            value={joinCode}
          />
          <p className="mt-2 text-ink-dull text-xs">
            Paste the full code or type the 12 words separated by hyphens
          </p>
        </div>

        {/* Node ID Input (optional, for cross-network) */}
        <div>
          <label className="mb-2 block font-medium text-ink-dull text-xs uppercase tracking-wider">
            Node ID <span className="text-ink-faint">(optional)</span>
          </label>
          <input
            className="w-full rounded-lg border border-sidebar-line bg-sidebar-box px-4 py-2.5 font-mono text-ink text-xs placeholder:text-ink-faint focus:outline-none focus:ring-2 focus:ring-accent/50 disabled:opacity-50"
            disabled={isLoading || !!state}
            onChange={(e) => setJoinNodeId(e.target.value)}
            placeholder="Enter Node ID for cross-network pairing"
            type="text"
            value={joinNodeId}
          />
          <p className="mt-1.5 text-ink-dull text-xs">
            Required if devices are on different networks
          </p>
        </div>

        {/* Status */}
        {state && (
          <div className="flex items-center gap-2 rounded-lg border border-app-line bg-app-box/40 p-3">
            <div className="size-2 animate-pulse rounded-full bg-accent" />
            <span className="text-ink-dull text-sm">
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
            className="flex-1 rounded-lg border border-app-line bg-app-box px-4 py-2.5 font-medium text-ink-dull text-sm transition-colors hover:bg-app-hover hover:text-ink"
            onClick={onCancel}
          >
            Cancel
          </button>
        ) : (
          <>
            <button
              className="rounded-lg border border-app-line bg-app-box px-6 py-2.5 font-medium text-ink-dull text-sm transition-colors hover:bg-app-hover hover:text-ink"
              onClick={onCancel}
            >
              Clear
            </button>
            <button
              className={clsx(
                "flex flex-1 items-center justify-center gap-2 rounded-lg px-4 py-2.5 font-medium transition-colors",
                isLoading || !joinCode.trim()
                  ? "cursor-not-allowed bg-accent/50 text-white/70"
                  : "bg-accent text-white hover:bg-accent/90"
              )}
              disabled={isLoading || !joinCode.trim()}
              onClick={onJoin}
            >
              {isLoading && (
                <ArrowsClockwise
                  className="size-5 animate-spin"
                  weight="bold"
                />
              )}
              {isLoading ? "Joining..." : "Join"}
            </button>
          </>
        )}
      </div>

      {/* Error */}
      {joinPairing.isError && (
        <div className="flex items-start gap-2 rounded-lg border border-red-500/30 bg-red-500/10 p-3">
          <Warning
            className="mt-0.5 size-5 shrink-0 text-red-500"
            weight="bold"
          />
          <div className="flex-1">
            <p className="font-medium text-red-500 text-sm">Failed to join</p>
            <p className="mt-0.5 text-ink-dull text-xs">
              {String(joinPairing.error)}
            </p>
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
