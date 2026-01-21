import { motion } from "framer-motion";
import { Ball } from "@sd/assets/images";
import Orb from "../../components/Orb";
import { TopBarButton } from "@sd/ui";
import { GlobeHemisphereWest, GithubLogo, DiscordLogo } from "@phosphor-icons/react";

export function AboutSettings() {

  return (
    <div className="flex flex-col items-center justify-center min-h-[600px]">
      {/* Animated orb with ball */}
      <motion.div
        initial={{ scale: 0.8, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        transition={{ duration: 0.6, ease: "easeOut" }}
        className="relative w-64 h-64 mb-8"
      >
        {/* Ball image - behind the orb */}
        <div className="absolute inset-[8%] z-0">
          <img
            src={Ball}
            alt="Spacedrive"
            className="w-full h-full object-contain select-none"
            draggable={false}
          />
        </div>
        {/* Orb animation - inset to make it smaller */}
        <div className="absolute inset-[15%] z-10">
          <Orb
            hue={-30}
            hoverIntensity={0}
            rotateOnHover={false}
            forceHoverState={true}
          />
        </div>
      </motion.div>

      {/* Branding */}
      <motion.div
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.5, delay: 0.3 }}
        className="text-center mb-6"
      >
        <h3 className="text-2xl font-bold text-white mb-2">Spacedrive</h3>
        <p className="text-sm text-white/60">
          A file explorer from the future.
        </p>
      </motion.div>

      {/* Manifesto */}
      <motion.div
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.5, delay: 0.35 }}
        className="max-w-md text-center mb-8 px-4"
      >
        <p className="text-sm text-white/70 leading-relaxed">
          Infrastructure for the next era of computing. An architecture designed for multi-device environments from the ground up—not cloud services retrofitted with offline support, but local-first sync that scales to the cloud when you want it.
        </p>
      </motion.div>

      {/* Links */}
      <motion.div
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.5, delay: 0.4 }}
        className="flex gap-3 mb-6"
      >
        <a
          href="https://spacedrive.com"
          target="_blank"
          rel="noopener noreferrer"
        >
          <TopBarButton icon={GlobeHemisphereWest}>
            Website
          </TopBarButton>
        </a>
        <a
          href="https://github.com/spacedriveapp/spacedrive"
          target="_blank"
          rel="noopener noreferrer"
        >
          <TopBarButton icon={GithubLogo}>
            GitHub
          </TopBarButton>
        </a>
        <a
          href="https://discord.gg/spacedrive"
          target="_blank"
          rel="noopener noreferrer"
        >
          <TopBarButton icon={DiscordLogo}>
            Discord
          </TopBarButton>
        </a>
      </motion.div>

      {/* License */}
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ duration: 0.5, delay: 0.5 }}
        className="text-center"
      >
        <a
          href="https://github.com/spacedriveapp/spacedrive/blob/main/LICENSE"
          target="_blank"
          rel="noopener noreferrer"
          className="text-sm text-white/40 hover:text-white/60 transition-colors"
        >
          AGPL-3.0
        </a>
      </motion.div>
    </div>
  );
}
