import { AnimatePresence, motion } from "framer-motion";

interface TabContentProps {
  id: string;
  activeTab: string;
  children: React.ReactNode;
}

export function TabContent({ id, activeTab, children }: TabContentProps) {
  if (id !== activeTab) return null;

  return (
    <AnimatePresence mode="wait">
      <motion.div
        animate={{ opacity: 1, y: 0 }}
        className="flex-1 overflow-auto"
        exit={{ opacity: 0, y: -10 }}
        initial={{ opacity: 0, y: 10 }}
        key={id}
        transition={{ duration: 0.15, ease: [0.25, 1, 0.5, 1] }}
      >
        {children}
      </motion.div>
    </AnimatePresence>
  );
}
