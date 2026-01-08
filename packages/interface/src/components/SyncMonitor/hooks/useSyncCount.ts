import { useEffect, useState } from "react";
import { useLibraryQuery } from "../../../contexts/SpacedriveContext";

export function useSyncCount() {
  const [onlinePeerCount, setOnlinePeerCount] = useState(0);
  const [isSyncing, setIsSyncing] = useState(false);

  const { data } = useLibraryQuery({
    type: "sync.activity",
    input: {},
  });

  useEffect(() => {
    if (data) {
      const onlineCount = data.peers.filter((p) => p.isOnline).length;
      setOnlinePeerCount(onlineCount);

      const state = data.currentState;
      const syncing =
        typeof state === "object" &&
        ("Backfilling" in state || "CatchingUp" in state);
      setIsSyncing(syncing);
    }
  }, [data]);

  return { onlinePeerCount, isSyncing };
}
