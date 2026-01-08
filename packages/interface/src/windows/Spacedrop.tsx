import { ShareNetwork, Users, X } from "@phosphor-icons/react";
import { TopBarButton } from "@sd/ui";
import { motion } from "framer-motion";
import { useMemo, useState } from "react";

interface Person {
  id: string;
  name: string;
  initials: string;
  status: "online" | "offline";
}

interface SpacedropProps {
  onClose?: () => void;
  people?: Person[];
}

export function Spacedrop({ onClose, people = [] }: SpacedropProps) {
  const [selectedPerson, setSelectedPerson] = useState<string | null>(null);
  const [starSpeed, setStarSpeed] = useState(1);

  // Generate star positions
  const stars = useMemo(
    () =>
      Array.from({ length: 50 }, (_, i) => ({
        id: i,
        x: Math.random() * 100,
        y: Math.random() * 100,
        size: Math.random() * 2 + 1,
        duration: Math.random() * 3 + 2,
      })),
    []
  );

  const handlePersonSelect = (id: string) => {
    setSelectedPerson(id);
    setStarSpeed(3);
    setTimeout(() => setStarSpeed(1), 2000);
  };

  return (
    <div className="relative flex h-full w-full flex-col overflow-hidden rounded-2xl border border-app-line bg-black">
      {/* Animated Stars Background */}
      <div className="absolute inset-0">
        {stars.map((star) => (
          <motion.div
            animate={{
              opacity: [0.2, 1, 0.2],
              scale: [1, 1.2, 1],
            }}
            className="absolute rounded-full bg-white"
            key={star.id}
            style={{
              left: `${star.x}%`,
              top: `${star.y}%`,
              width: star.size,
              height: star.size,
            }}
            transition={{
              duration: star.duration / starSpeed,
              repeat: Number.POSITIVE_INFINITY,
              ease: "easeInOut",
            }}
          />
        ))}
      </div>

      {/* Top Bar */}
      <div className="relative z-10 flex items-center justify-between border-app-line/30 border-b bg-app/80 p-3 backdrop-blur-xl">
        <div className="flex gap-2">
          <TopBarButton icon={X} onClick={onClose}>
            Close
          </TopBarButton>
        </div>

        <div className="flex items-center gap-2">
          <TopBarButton icon={Users}>
            {people.length} {people.length === 1 ? "Device" : "Devices"}
          </TopBarButton>
          <TopBarButton icon={ShareNetwork}>Share</TopBarButton>
        </div>
      </div>

      {/* Content */}
      <div className="relative z-10 flex flex-1 items-center justify-center p-8">
        {people.length === 0 ? (
          <motion.div
            animate={{ opacity: 1, y: 0 }}
            className="text-center"
            initial={{ opacity: 0, y: 20 }}
          >
            <div className="mb-4 flex justify-center">
              <div className="rounded-full bg-sidebar-box/40 p-6 backdrop-blur-lg">
                <Users className="size-12 text-sidebar-inkFaint" />
              </div>
            </div>
            <h2 className="mb-2 font-semibold text-lg text-sidebar-ink">
              No devices found
            </h2>
            <p className="text-sidebar-inkFaint text-sm">
              Waiting for nearby devices...
            </p>
          </motion.div>
        ) : (
          <div className="grid max-w-4xl grid-cols-2 gap-6 sm:grid-cols-3 lg:grid-cols-4">
            {people.map((person, index) => (
              <motion.button
                animate={{ opacity: 1, scale: 1 }}
                className="group relative flex flex-col items-center gap-3 rounded-xl border border-sidebar-line/30 bg-sidebar-box/40 p-4 backdrop-blur-lg transition-all hover:border-accent/50 hover:bg-sidebar-box/60"
                initial={{ opacity: 0, scale: 0.8 }}
                key={person.id}
                onClick={() => handlePersonSelect(person.id)}
                transition={{
                  duration: 0.3,
                  delay: index * 0.08,
                  ease: [0.25, 1, 0.5, 1],
                }}
              >
                {/* Selection Indicator */}
                {selectedPerson === person.id && (
                  <motion.div
                    className="absolute inset-0 rounded-xl border-2 border-accent bg-accent/10"
                    layoutId="selection"
                    transition={{
                      duration: 0.2,
                      ease: [0.25, 1, 0.5, 1],
                    }}
                  />
                )}

                {/* Avatar */}
                <div className="relative">
                  <div className="flex size-16 items-center justify-center rounded-full bg-accent/20 font-bold text-accent text-xl">
                    {person.initials}
                  </div>

                  {/* Status Badge */}
                  <div className="absolute -right-1 -bottom-1">
                    {person.status === "online" ? (
                      <div className="relative">
                        <div className="size-4 rounded-full border-2 border-sidebar-box bg-green-500" />
                        <motion.div
                          animate={{
                            scale: [1, 1.4, 1],
                            opacity: [0.6, 0, 0.6],
                          }}
                          className="absolute inset-0 rounded-full bg-green-500"
                          transition={{
                            duration: 2,
                            repeat: Number.POSITIVE_INFINITY,
                          }}
                        />
                      </div>
                    ) : (
                      <div className="size-4 rounded-full border-2 border-sidebar-box bg-sidebar-inkFaint" />
                    )}
                  </div>
                </div>

                {/* Name */}
                <div className="text-center">
                  <p className="font-medium text-sidebar-ink text-sm group-hover:text-accent">
                    {person.name}
                  </p>
                  <p className="text-sidebar-inkFaint text-xs">
                    {person.status === "online" ? "Online" : "Offline"}
                  </p>
                </div>
              </motion.button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
