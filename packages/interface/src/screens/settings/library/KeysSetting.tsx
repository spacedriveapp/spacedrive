import { Button } from "@sd/ui";
import { ListKeys } from "../../../components/key/KeyList";
import { KeyMounter } from "../../../components/key/KeyMounter";
import { SettingsContainer } from "../../../components/settings/SettingsContainer";
import { SettingsHeader } from "../../../components/settings/SettingsHeader";
import clsx from 'clsx';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import { PropsWithChildren, useState } from 'react';
import { animated, useTransition } from 'react-spring';

interface Props extends DropdownMenu.MenuContentProps {
	trigger: React.ReactNode;
	transformOrigin?: string;
	disabled?: boolean;
}

export const KeyMounterDropdown = ({
	trigger,
	children,
	disabled,
	transformOrigin,
	className,
	...props
}: PropsWithChildren<Props>) => {
	const [open, setOpen] = useState(false);

	const transitions = useTransition(open, {
	  from: {
		opacity: 0,
		transform: `scale(0.9)`,
		transformOrigin: transformOrigin || "top",
	  },
	  enter: { opacity: 1, transform: "scale(1)" },
	  leave: { opacity: -0.5, transform: "scale(0.95)" },
	  config: { mass: 0.4, tension: 200, friction: 10 },
	});
	
	return (
	  <DropdownMenu.Root open={open} onOpenChange={setOpen}>
		<DropdownMenu.Trigger>{trigger}</DropdownMenu.Trigger>
		{transitions(
		  (styles, show) =>
			show && (
			  <DropdownMenu.Portal forceMount>
				<DropdownMenu.Content forceMount asChild>
				  <animated.div
					// most of this is copied over from the `OverlayPanel`
					className={clsx(
					  "flex flex-col",
					  "z-50 m-2 space-y-1",
					  "select-none cursor-default rounded-lg",
					  "text-left text-sm text-ink",
					  "bg-app-overlay/80 backdrop-blur",
					  // 'border border-app-overlay',
					  "shadow-2xl shadow-black/60 ",
					  className
					)}
					style={styles}
				  >
					{children}
				  </animated.div>
				</DropdownMenu.Content>
			  </DropdownMenu.Portal>
			)
		)}
	  </DropdownMenu.Root>
	);	
};

export default function KeysSettings() {
  return (
    <SettingsContainer>
      <SettingsHeader
        title="Keys"
        description="Manage your keys."
        rightArea={
          <div className="flex flex-row items-center space-x-5">
            <KeyMounterDropdown
              trigger={
                <Button variant="accent" size="sm" onClick={() => {}}>
                  Add Key
                </Button>
              }
            >
              <KeyMounter />
            </KeyMounterDropdown>
          </div>
        }
      />
      <div className="grid space-y-2">{ListKeys(false)}</div>
    </SettingsContainer>
  );
}
