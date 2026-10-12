import { useRef } from "react";
import type { RegionRectV1 } from "../contracts";
const clamp = (n: number, max = 1) => Math.max(0, Math.min(max, n));
export function RegionEditor({
  src,
  width,
  height,
  rectangle,
  onChange,
}: {
  src: string;
  width: number;
  height: number;
  rectangle: RegionRectV1;
  onChange: (r: RegionRectV1) => void;
}) {
  const drag = useRef<{
    x: number;
    y: number;
    mode: string;
    original: RegionRectV1;
  } | null>(null);
  function point(e: React.PointerEvent<SVGSVGElement>) {
    const m = e.currentTarget.getScreenCTM();
    if (!m) return null;
    const p = e.currentTarget.createSVGPoint();
    p.x = e.clientX;
    p.y = e.clientY;
    const v = p.matrixTransform(m.inverse());
    return { x: clamp(v.x / width, 0.999), y: clamp(v.y / height, 0.999) };
  }
  const handle = Math.max(width, height) / 70;
  return (
    <svg
      className="canvas-region-editor"
      viewBox={`0 0 ${width} ${height}`}
      role="img"
      aria-label="Draw crop on frame; drag inside to move, drag corner to resize. Numeric fields are available in Cloud AI."
      onPointerDown={(e) => {
        const p = point(e);
        if (!p) return;
        e.stopPropagation();
        drag.current = {
          ...p,
          mode:
            rectangle.width >= 0.999 && rectangle.height >= 0.999
              ? "draw"
              : ((e.target as Element).getAttribute("data-crop-action") ??
                "draw"),
          original: rectangle,
        };
        e.currentTarget.setPointerCapture(e.pointerId);
      }}
      onPointerMove={(e) => {
        const p = point(e),
          d = drag.current;
        if (!p || !d) return;
        e.stopPropagation();
        if (d.mode === "move")
          onChange({
            ...d.original,
            x: clamp(d.original.x + p.x - d.x, 1 - d.original.width),
            y: clamp(d.original.y + p.y - d.y, 1 - d.original.height),
          });
        else if (d.mode === "resize")
          onChange({
            ...d.original,
            width: Math.max(0.001, p.x - d.original.x),
            height: Math.max(0.001, p.y - d.original.y),
          });
        else
          onChange({
            x: Math.min(p.x, d.x),
            y: Math.min(p.y, d.y),
            width: Math.max(0.001, Math.abs(p.x - d.x)),
            height: Math.max(0.001, Math.abs(p.y - d.y)),
          });
      }}
      onPointerUp={() => {
        drag.current = null;
      }}
      onPointerCancel={() => {
        drag.current = null;
      }}
    >
      <image href={src} width={width} height={height} />
      <rect
        data-crop-action="move"
        x={rectangle.x * width}
        y={rectangle.y * height}
        width={rectangle.width * width}
        height={rectangle.height * height}
        fill="rgba(15,108,189,.15)"
        stroke="var(--color-focus)"
        strokeWidth={Math.max(width, height) / 250}
      />
      <rect
        data-crop-action="resize"
        x={(rectangle.x + rectangle.width) * width - handle}
        y={(rectangle.y + rectangle.height) * height - handle}
        width={handle}
        height={handle}
        fill="var(--color-focus)"
      />
    </svg>
  );
}
