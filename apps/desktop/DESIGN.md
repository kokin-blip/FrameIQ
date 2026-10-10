---
name: FrameIQ Fluent Docked Editor
description: Matte graphite desktop tools around an opaque frame canvas.
colors:
  background: "#1b1b1b"
  surface: "#202020"
  raised: "#292929"
  border: "#3d3d3d"
  text: "#f5f5f5"
  muted: "#bdbdbd"
  accent: "#0f6cbd"
  accent-hover: "#115ea3"
  focus: "#479ef5"
  ink: "#ffffff"
  danger: "#ffb4ab"
  canvas: "#141414"
  hover: "#333333"
  selected: "#253544"
  error-surface: "#382324"
typography:
  headline:
    fontFamily: "Inter, sans-serif"
    fontSize: "28px"
    fontWeight: 600
    lineHeight: 1.3
    letterSpacing: "-0.025em"
  title:
    fontFamily: "Inter, sans-serif"
    fontSize: "16px"
    fontWeight: 600
    lineHeight: 1.5
  body:
    fontFamily: "Inter, sans-serif"
    fontSize: "14px"
    lineHeight: 1.6
  label:
    fontFamily: "Inter, sans-serif"
    fontSize: "13px"
    fontWeight: 500
rounded:
  control: "4px"
  panel: "6px"
spacing:
  space-1: "4px"
  space-2: "8px"
  space-3: "12px"
  space-4: "16px"
  space-5: "20px"
  space-6: "24px"
components:
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.ink}"
    typography: "{typography.label}"
    rounded: "{rounded.control}"
    padding: "7px 12px"
  button-primary-hover:
    backgroundColor: "{colors.accent-hover}"
  button-secondary:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.text}"
    typography: "{typography.label}"
    rounded: "{rounded.control}"
    padding: "7px 12px"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.muted}"
    rounded: "{rounded.control}"
    padding: "7px 12px"
  input:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.text}"
    rounded: "{rounded.control}"
    padding: "8px 12px"
  workspace-switch:
    backgroundColor: "transparent"
    textColor: "{colors.muted}"
    rounded: "{rounded.control}"
    padding: "7px 12px"
  status-badge:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.muted}"
    rounded: "{rounded.control}"
    padding: "4px 8px"
  panel:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.panel}"
  frame-card:
    backgroundColor: "transparent"
    textColor: "{colors.text}"
    rounded: "{rounded.panel}"
    padding: "9px"
  canvas:
    backgroundColor: "{colors.canvas}"
  attachment:
    backgroundColor: "transparent"
    textColor: "{colors.text}"
    rounded: "{rounded.panel}"
---
# Design System: FrameIQ

## Overview

**Creative North Star: "Fluent Docked Editor"**

FrameIQ follows the approved Fluent Docked Editor direction: matte graphite surfaces, compact Inter typography, outlined icons, and restrained blue actions. Image inspection remains central, with direct access to frame identity and supporting controls.

The shared visual system applies to Studio, Library, Export, and Settings. Tonal surface steps and fine dividers organize information; imagery comes from imported media. The approved reference is docs/design/mockups/fluent-option-1.png. The previous Canvas/DesignCode world is superseded; its handoffs, captures, and earlier Figma pages remain historical references.

**Key Characteristics:**

- Opaque graphite surfaces with no decorative glow or blur.
- Compact Inter controls and tabular frame/time numerals.
- Docked tools with a large neutral image inspection canvas.
- Consistent outlined Lucide icons with a 1.6px stroke and distinct keyboard focus.

## Colors

The graphite neutral palette supports blue actions and clear reading hierarchy. Frontmatter values match `packages/ui/src/tokens.css`; the sidecar adds display metadata and component states.

### Primary

- **Fluent Action Blue** (`accent`, `accent-hover`): primary selection/export actions and text selection. Use white contrast text (`ink`).
- **Focus Blue** (`focus`): keyboard outline, timeline selection, wheel marker, and selected frame edge. Focus and action fill have distinct roles.

### Tertiary

- **Error Rose** (`danger`, `error-surface`): readable failure messages on an opaque error background.

### Neutral

- **Graphite Ground** (`background`): application base and secondary pages.
- **Dock Graphite** (`surface`): header, toolbar, timeline, and selections/details dock.
- **Raised Graphite** (`raised`, `hover`): fields, secondary actions, active navigation, and hover feedback.
- **Selection Slate** (`selected`): selected frame background alongside a focus-blue edge.
- **Graphite Divider** (`border`): dock boundaries and control outlines.
- **Inspection Charcoal** (`canvas`): opaque image inspection and thumbnail ground.
- **Reading White / Secondary Gray** (`text`, `muted`): primary labels and supporting metadata.

**The Image Fit Rule.** Keep the inspection canvas opaque and fit the complete frame without distorting its aspect ratio. Zoom is relative to the fitted view.

## Typography

**Display and Body Font:** locally bundled Inter with sans-serif fallback. Labels share that family. Use tabular figures for timestamps, frame ordinals, metadata, and zoom percentages.

Page headings use the frontmatter headline role; section headings use title; paragraph text uses body; standard action labels use label. Supporting metadata uses observed sizes from 9–12px. Preserve this density for inspection data; avoid treating tiny thumbnail captions as general prose guidance.

The attachment instruction uses 22px, weight 500. Brand text uses 21px, weight 600, and -0.025em tracking; at the compact width it becomes 19px. Settings and expanded precision guidance limit paragraph width to 75ch. Standard paragraph line height is 1.6.

## Layout

The desktop shell fills the available window and has a 1000px minimum width. The reference capture sizes are 1440×960 and 1000×700; there is no mobile composition. Header height is 58px with 20px horizontal padding. The separate docked toolbar is at least 64px high. Studio uses a flexible viewer/timeline column and a fixed 300px selections/details column. The canvas has a 150px minimum height and contains aspect-preserving image fit, zoom, and pan.

The selections list scrolls independently. Metadata remains in the dock below the list, and the bottom export action stays available. Timeline thumbnails, scrubber, and precision wheel occupy the left column below the viewer. Background activity opens through its separate bottom activity strip; that strip carries processing state rather than a privacy badge.

At widths ≤1100px, header height becomes 52px, toolbar minimum height 56px, horizontal padding 14px, and the right dock 260px. Thumbnail height reduces from 68px to 52px; frame list thumbnails reduce from 96×66px to 80×56px. At heights ≤740px, timeline and dock spacing compact further. Keep the left viewer flexible and the dock accessible at these sizes.

Library uses an auto-filled card grid with 260px minimum card width. Export uses a 300px configuration column (280px compact) beside an auto-filled selected-frame grid. Settings content has a 1000px maximum width. Secondary pages scroll within the shared shell, with 32px/36px page padding reduced to 24px at compact widths. Shared spacing primitives use the established 4px rhythm; source-specific 10px, 14px, and 18px layouts remain intentional observed details.

## Elevation & Depth

Depth comes from opaque tonal layers and one-pixel boundaries. Docked surfaces and standard controls are flat. The floating activity panel uses `shadow-panel`; its exact value lives in the sidecar. There is no decorative glow, gradient, or backdrop blur, so no blur fallback is needed.

**The Flat at Rest Rule.** Separate docked surfaces with tone and borders. Reserve the panel shadow for the floating activity panel.

Buttons transition background and border color in 120ms ease-out. Reduced-motion preference disables transitions and animations, including descendant and pseudo-element treatments.

## Shapes

Controls use modest four-pixel corners (`control`); panels and frame rows use six-pixel corners (`panel`). Thumbnails and range thumbs use observed three-pixel corners where necessary. Dock boundaries are straight and structural; avoid inflated pill silhouettes. Outlined Lucide icons share a 1.6px stroke. These are local Fluent-style adaptations of the existing icon set.

## Components

### Buttons

Compact rectangular actions have a 36px minimum height, 7px/12px padding, and label typography. Primary fill uses action blue with white text; hover deepens the blue. Secondary actions use raised graphite and a divider border. Text and icon actions are transparent at rest, with neutral hover feedback. Standard icon buttons are 36px square. The dock export action uses a 42px minimum height and fills its available width.

Keyboard focus is a two-pixel focus-blue outline with a two-pixel offset. Disabled buttons and fields use 0.45 opacity and a default cursor. Keep hover, selection, and keyboard focus visually distinct.

### Inputs / Fields

Fields use raised graphite, a one-pixel divider border, control corners, and 8px/12px padding. Timestamp entry is 145px wide, 36px high, centered, and uses tabular numerals. The caret uses focus blue. Native source and export selects preserve their meaningful labels and supplied options.

### Navigation

The shared header holds brand, project/source context, workspace navigation, and actions. Studio/Export switching uses compact text buttons; active navigation uses raised graphite with primary reading text. Long project/source labels truncate within their allocated width rather than enlarging the shell.

### Chips / Status

Status badges and selection counts use raised graphite, secondary reading text, and control corners. They report factual state. Keep unavailable module labels explicit in Settings.

### Cards / Containers

Library cards use opaque dock graphite, a one-pixel divider, and panel corners. Studio selection rows combine an aspect-preserving thumbnail with frame ordinal, timestamp, and source. Hover raises the tone; selection adds slate fill and a focus-blue border. Export uses the same frame identity in a larger grid card. Images and metadata represent actual imported content.

### Image Canvas and Timeline

The viewer uses image fit as its baseline; zoom/pan affects inspection rather than output resolution. The direct attachment area says “Drop a video here” / “or click to attach.” Timeline thumbnails may crop for navigation, while the main viewer and selected-frame thumbnails fit the full frame. The timeline keeps exact selection, approximate playback, precision stepping, and source duration understandable.

### Provenance and Verification

Figma page **10 Fluent Docked Editor** was created before code changes in file `5L64rf754pinHPHimkme5e`. The official Fluent community kit was unavailable as an editable connected library; the Figma components are local editable adaptations of inspected official previews and the approved reference. See `docs/design/fluent-state.json`, `fluent-handoff.md`, and `fluent-feature-parity.md` for node mapping, implementation, and evidence.

Browser empty-state captures cover all four views at both reference sizes in `docs/design/review/fluent/`; the supplied reviewer found no material visual defect. TypeScript, seven frontend tests, production frontend build, and macOS bundle build passed. A locked Mac blocks complete native UI QA. Populated native workflows, Windows rendering/dialogs, physical trackpad behavior, native reduced-motion toggling, and cancellation under native load remain unverified. Earlier Canvas checks are historical evidence. No canonical composition-diff artifact or separate canonical spec was created by this workflow; the approved image, implementation, and handoff remain the available authority.

## Do's and Don'ts

### Do:

- **Do** use shared tokens and preserve full-frame image fit on the opaque canvas.
- **Do** keep project context, exact frame identities, selections, and metadata accessible.
- **Do** distinguish approximate playback from resolved selection and export.
- **Do** preserve visible focus, labels, reduced motion, and factual Settings copy.

### Don't:

- **Don't** add decorative glow, gradients, blur, or translucent dock surfaces.
- **Don't** restore floating playback tools or collapsible selection/detail panels from the superseded Canvas composition.
- **Don't** add a permanent privacy shield or local-processing footer badge.
- **Don't** claim official Fluent kit imports; components are local editable adaptations.
- **Don't** present unavailable AI or shopping modules as enabled, or use invented result imagery.
