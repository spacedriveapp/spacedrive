import { PositionContext } from "./Item";

interface TopBarPortalProps {
  left?: React.ReactNode;
  center?: React.ReactNode;
  right?: React.ReactNode;
}

export function TopBarPortal({ left, center, right }: TopBarPortalProps) {
  return (
    <>
      {left && (
        <PositionContext.Provider value="left">{left}</PositionContext.Provider>
      )}
      {center && (
        <PositionContext.Provider value="center">
          {center}
        </PositionContext.Provider>
      )}
      {right && (
        <PositionContext.Provider value="right">
          {right}
        </PositionContext.Provider>
      )}
    </>
  );
}
