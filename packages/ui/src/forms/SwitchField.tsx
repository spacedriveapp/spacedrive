import clsx from "clsx";
import { forwardRef } from "react";
import { useController } from "react-hook-form";

import { Switch, type SwitchProps } from "../Switch";
import { FormField, type UseFormFieldProps, useFormField } from "./FormField";

export interface SwitchFieldProps extends UseFormFieldProps, SwitchProps {
  name: string;
}

export const SwitchField = forwardRef<HTMLButtonElement, SwitchFieldProps>(
  (props, ref) => {
    const { field } = useController(props);
    const { formFieldProps, childProps } = useFormField(props);

    return (
      <FormField {...formFieldProps}>
        <Switch
          {...childProps}
          checked={field.value}
          className={clsx(props.disabled ? "opacity-60" : undefined)}
          onCheckedChange={field.onChange}
          ref={ref}
        />
      </FormField>
    );
  }
);
