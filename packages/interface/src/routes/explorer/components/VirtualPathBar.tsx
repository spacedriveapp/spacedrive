import LaptopIcon from "@sd/assets/icons/Laptop.png";
import clsx from "clsx";
import { motion } from "framer-motion";
import { getDeviceIcon } from "../../../contexts/SpacedriveContext";
import type { VirtualView } from "../context";

interface VirtualPathBarProps {
  view: VirtualView;
  devices: Map<string, any>;
}

/**
 * PathBar for virtual views (device listings, all devices, etc.)
 * Shows the view name with an appropriate icon instead of file path breadcrumbs
 */
export function VirtualPathBar({ view, devices }: VirtualPathBarProps) {
  const device = view.id ? devices.get(view.id) : null;

  // Determine label and icon based on view type
  const label = (() => {
    if (view.view === "device" && device) {
      return device.name;
    }
    if (view.view === "devices") {
      return "All Devices";
    }
    return "Virtual View";
  })();

  const icon = (() => {
    if (view.view === "device" && device) {
      return getDeviceIcon(device);
    }
    return LaptopIcon;
  })();

  return (
    <motion.div
      animate={{ width: Math.min(label.length * 8.5 + 70, 400) }}
      className={clsx(
        "flex h-8 items-center gap-1.5 rounded-full px-3",
        "border border-sidebar-line/30 backdrop-blur-xl",
        "bg-sidebar-box/20 transition-colors"
      )}
      initial={{ width: 150 }}
      transition={{ duration: 0.2, ease: [0.25, 1, 0.5, 1] }}
    >
      <img alt="" className="size-5 flex-shrink-0 opacity-60" src={icon} />
      <span className="whitespace-nowrap font-medium text-sidebar-ink text-xs">
        {label}
      </span>
    </motion.div>
  );
}
