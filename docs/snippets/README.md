# Custom Mintlify Components

This directory contains reusable React components for Spacedrive documentation.

## Available Components

### FlowDiagram

A flow diagram component for visualizing multi-step processes with Spacedrive styling.

**Usage:**

```mdx
import { FlowDiagram } from '/snippets/FlowDiagram.mdx';

<FlowDiagram steps={[
  {
    title: "Step title",
    description: "Step description",
    details: ["Detail 1", "Detail 2"],
    metrics: { "Metric name": "value" }
  }
]} />
```

**Props:**

- `steps` (array, required): Array of step objects
  - `title` (string, required): Step title
  - `description` (string, optional): Step description
  - `details` (string[], optional): Bullet points for additional details
  - `metrics` (object, optional): Key-value pairs displayed as metric badges

**Example:**

See `docs/react/ui/normalized-cache.mdx` for a real-world example.

## Adding New Components

1. Create a new `.mdx` file in this directory
2. Export your React component
3. Import and use it in any documentation page with `import { Component } from '/snippets/Component.mdx'`

## Styling

Components use Tailwind CSS classes and Spacedrive's accent color (`#36A3FF`).
