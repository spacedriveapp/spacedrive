import { CircleNotch } from "@phosphor-icons/react";
import { Ball } from "@sd/assets/images";
import { AnimatePresence, motion } from "framer-motion";
import { useEffect, useState } from "react";
import { usePlatform } from "../../contexts/PlatformContext";
import Orb from "../Orb";

export function DaemonStartupOverlay({ show }: { show: boolean }) {
  const platform = usePlatform();
  const [version, setVersion] = useState<string>("2.0.0");
  const isDev = import.meta.env.DEV;

  // Get version from platform abstraction
  useEffect(() => {
    const getVersion = async () => {
      try {
        if (platform.getAppVersion) {
          const appVersion = await platform.getAppVersion();
          setVersion(appVersion);
        }
      } catch (e) {
        // Fallback if platform doesn't support version or API fails
        setVersion("2.0.0-pre.1");
      }
    };
    getVersion();
  }, [platform]);

  const versionText = isDev ? `${version} (dev)` : version;

  return (
    <AnimatePresence>
      {show && (
        <motion.div
          animate={{ opacity: 1 }}
          className="fixed inset-0 z-[9999] flex items-center justify-center bg-black"
          exit={{ opacity: 0 }}
          initial={{ opacity: 0 }}
          transition={{
            duration: 0.6,
            ease: "easeInOut",
          }}
        >
          {/* Animated orb with ball */}
          <motion.div
            animate={{ scale: 1, opacity: 1 }}
            className="relative h-64 w-64"
            exit={{ scale: 0.95, opacity: 0 }}
            initial={{ scale: 0.8, opacity: 0 }}
            transition={{ duration: 0.6, ease: "easeOut" }}
          >
            {/* Ball image - behind the orb */}
            <div className="absolute inset-[8%] z-0">
              <img
                alt="Spacedrive"
                className="h-full w-full select-none object-contain"
                draggable={false}
                src={Ball}
              />
            </div>
            {/* Orb animation - inset to make it smaller */}
            <div className="absolute inset-[15%] z-10">
              <Orb
                forceHoverState={true}
                hoverIntensity={0}
                hue={-30}
                rotateOnHover={false}
              />
            </div>
          </motion.div>

          {/* Loading text - bottom right */}
          <motion.div
            animate={{ opacity: 1, x: 0 }}
            className="fixed right-6 bottom-6 flex items-center gap-3"
            exit={{ opacity: 0, x: -10 }}
            initial={{ opacity: 0, x: 10 }}
            transition={{ duration: 0.5, delay: 0.3 }}
          >
            <CircleNotch
              className="size-5 animate-spin text-white"
              weight="bold"
            />
            <div className="flex flex-col">
              <p className="font-bold text-lg text-white">
                Starting Spacedrive
              </p>
              <p className="text-sm text-white/50">v{versionText}</p>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
