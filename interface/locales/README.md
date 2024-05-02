
# Locales

This directory contains the translations for the application.

## Adding a new language

To add a new language, create a new directory with the language code (e.g. `es` for Spanish) and copy the `en` directory into it. Then, translate the strings in the new directory.

To display the new language as an option in the application, add the language code to the `LANGUAGE_OPTIONS` array in `interface/app/$libraryId/settings/client/appearance.tsx`.

```ts
export const LANGUAGE_OPTIONS = [
  { value: 'en', label: 'English' },
  { value: 'es', label: 'Español' },
  // The rest of the languages
];
```

## Syncing locales

This command will help you sync locales with the source language (en) and find missing keys.

`npx i18next-locales-sync -p en -s it -l ./interface/locales`

replace `it` with the language you want to sync with the source language.
