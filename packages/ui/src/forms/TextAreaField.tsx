import { forwardRef } from "react";

import * as Root from "../Input";
import { FormField, type UseFormFieldProps, useFormField } from "./FormField";

export interface TextAreaFieldProps
  extends UseFormFieldProps,
    Root.TextareaProps {
  name: string;
}

export const TextAreaField = forwardRef<
  HTMLTextAreaElement,
  TextAreaFieldProps
>((props, ref) => {
  const { formFieldProps, childProps } = useFormField(props);

  return (
    <FormField {...formFieldProps}>
      <Root.TextArea
        {...childProps}
        error={formFieldProps.error !== undefined}
        ref={ref}
      />
    </FormField>
  );
});
