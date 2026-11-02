import { useRef, useState, useEffect } from "react";
import { api } from "../client";
import { RegionEditor } from "./RegionEditor";
import type { AssetRefV1, FashionItemV1, RegionRectV1 } from "../contracts";
export function FashionCanvas({
  asset,
  items,
  selected,
  onSelect,
  rectangle,
  onRectangle,
  polygon,
  onPolygon,
  mode,
  mask,
}: {
  asset: AssetRefV1;
  items: FashionItemV1[];
  selected: string;
  onSelect: (id: string) => void;
  rectangle: RegionRectV1;
  onRectangle: (r: RegionRectV1) => void;
  polygon: [number, number][];
  onPolygon: (p: [number, number][]) => void;
  mode: string;
  mask: boolean;
}) {
  const drawing = useRef(false);
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [asset.path]);
  if (failed)
    return (
      <div role="alert" className="fashion-empty">
        <h2>Image unavailable</h2>
        <p>
          The saved image could not be displayed. Select the source again or
          reimport the changed file.
        </p>
        <button onClick={() => setFailed(false)}>Retry image</button>
      </div>
    );
  if (mode === "box")
    return (
      <RegionEditor
        src={api.image(asset.path)}
        width={asset.width}
        height={asset.height}
        rectangle={rectangle}
        onChange={onRectangle}
      />
    );
  function point(
    e: React.PointerEvent<SVGSVGElement>,
  ): [number, number] | null {
    const m = e.currentTarget.getScreenCTM();
    if (!m) return null;
    const p = e.currentTarget.createSVGPoint();
    p.x = e.clientX;
    p.y = e.clientY;
    const q = p.matrixTransform(m.inverse());
    return [
      Math.max(0, Math.min(1, q.x / asset.width)),
      Math.max(0, Math.min(1, q.y / asset.height)),
    ];
  }
  const item = items.find((i) => i.id === selected);
  return (
    <svg
      className="canvas-region-editor fashion-canvas"
      viewBox={`0 0 ${asset.width} ${asset.height}`}
      role="img"
      aria-label="Fashion item inspection. Select an item in the item list for keyboard editing."
      onPointerDown={(e) => {
        if (mode !== "polygon" && mode !== "outline") return;
        const p = point(e);
        if (!p) return;
        e.currentTarget.setPointerCapture(e.pointerId);
        if (mode === "outline") {
          drawing.current = true;
          onPolygon([p]);
        } else onPolygon([...polygon, p].slice(-128));
      }}
      onPointerMove={(e) => {
        if (!drawing.current) return;
        const p = point(e);
        if (p && polygon.length < 128) onPolygon([...polygon, p]);
      }}
      onPointerUp={() => {
        drawing.current = false;
      }}
      onPointerCancel={() => {
        drawing.current = false;
      }}
    >
      <image
        href={api.image(asset.path)}
        onError={() => setFailed(true)}
        width={asset.width}
        height={asset.height}
      />
      {mask && item?.mask && (
        <image
          href={api.image(item.mask.path)}
          width={asset.width}
          height={asset.height}
          opacity=".3"
          style={{ mixBlendMode: "screen" }}
        />
      )}
      {items.map((i) => (
        <g
          key={i.id}
          onClick={() => onSelect(i.id)}
          style={{ cursor: "pointer" }}
        >
          <rect
            x={i.rectangle.x * asset.width}
            y={i.rectangle.y * asset.height}
            width={i.rectangle.width * asset.width}
            height={i.rectangle.height * asset.height}
            fill={i.id === selected ? "rgba(15,108,189,.16)" : "transparent"}
            stroke={
              i.id === selected ? "var(--color-focus)" : "var(--color-muted)"
            }
            strokeWidth={asset.width / 400}
          />
          <title>{i.category}</title>
        </g>
      ))}
      {polygon.length > 0 && (
        <polygon
          points={polygon
            .map(([x, y]) => `${x * asset.width},${y * asset.height}`)
            .join(" ")}
          fill="rgba(15,108,189,.2)"
          stroke="var(--color-focus)"
          strokeWidth={asset.width / 300}
        />
      )}
    </svg>
  );
}
