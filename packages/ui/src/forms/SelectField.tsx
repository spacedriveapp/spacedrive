import {
  type FieldValues,
  type UseControllerProps,
  useController,
} from "react-hook-form";

import * as Root from "../Select";
import { FormField, type UseFormFieldProps, useFormField } from "./FormField";

export interface SelectFieldProps<T extends FieldValues>
  extends Omit<UseFormFieldProps, "name">,
    Omit<Root.SelectProps, "value" | "onChange">,
    UseControllerProps<T> {}

export const SelectField = <T extends FieldValues>(
  props: SelectFieldProps<T>
) => {
  const { formFieldProps, childProps } = useFormField(props);
  const { field } = useController({ name: props.name });

  return (
    <FormField {...formFieldProps}>
      <Root.Select
        {...childProps}
        className="w-full"
        onChange={field.onChange}
        value={field.value}
      />
    </FormField>
  );
};
