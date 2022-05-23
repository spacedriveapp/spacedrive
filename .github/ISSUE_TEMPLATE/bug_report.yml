name: 🐞 Bug Report
description: Report a bug
labels:
  - kind/bug
  - status/needs-triage

body:
  - type: markdown
    attributes:
      value: |
        ## First of all
        1. Please search for [existing issues](https://github.com/spacedriveapp/spacedrive/issues?q=is%3Aissue) about this problem first.
        2. Make sure you run have the latest version of Rust (`rustup update`) and PNPM (`pnpm add -g pnpm`) along with all relevant Spacedrive dependencies.
        3. Make sure it's an issue with Spacedrive and not something else you are using.
        4. Remember to follow our community guidelines and be friendly.

  - type: textarea
    id: description
    attributes:
      label: Describe the bug
      description: A clear description of what the bug is. Include screenshots if applicable.
      placeholder: Bug description
    validations:
      required: true

  - type: textarea
    id: reproduction
    attributes:
      label: Reproduction
      description: Steps to reproduce the behavior.
      placeholder: |
        1. Go to ...
        2. Click on ...
        3. See error

  - type: textarea
    id: expected-behavior
    attributes:
      label: Expected behavior
      description: A clear description of what you expected to happen.

  - type: textarea
    id: info
    attributes:
      label: Platform and versions
      description: 'Please include the output of `pnpm --version && cargo --version && rustc --version` along with information about your Operating System such as version and/or specific distribution if revelant.'
      render: Shell
    validations:
      required: true

  - type: textarea
    id: logs
    attributes:
      label: Stack trace
      render: Shell

  - type: textarea
    id: context
    attributes:
      label: Additional context
      description: Add any other context about the problem here.
