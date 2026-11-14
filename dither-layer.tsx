import { useEffect, useMemo, useState } from "react";
import { createDitherTexture, type DitherOptions } from "./dither";

/** A cached native texture behind content, with no input or animation loop. */
export function DitherLayer({
  variant = "ambient",
  strength = 1,
  opacity = 1,
  radius = 0,
  value = 1,
}: {
  variant?: DitherOptions["variant"];
  strength?: number;
  opacity?: number;
  radius?: number;
  value?: number;
}) {
  const [bounds, setBounds] = useState({ width: 0, height: 0 });
  const [size, setSize] = useState(bounds);
  // Window resizing can issue many layout events. Keep the last cached texture
  // stretched during a resize and rasterize once the surface settles.
  useEffect(() => {
    const timer = setTimeout(() => setSize(bounds), 80);
    return () => clearTimeout(timer);
  }, [bounds]);
  const source = useMemo(
    () =>
      size.width && size.height
        ? createDitherTexture({ ...size, variant, strength, value })
        : null,
    [size, variant, strength, value],
  );
  return (
    <div
      role="presentation"
      onLayout={(event) => {
        const width = Math.max(0, Math.ceil(event.width ?? 0));
        const height = Math.max(0, Math.ceil(event.height ?? 0));
        setBounds((current) =>
          current.width === width && current.height === height
            ? current
            : { width, height },
        );
      }}
      style={{
        position: "absolute",
        top: 0,
        right: 0,
        bottom: 0,
        left: 0,
        borderRadius: radius,
        overflow: "hidden",
        pointerEvents: "none",
        opacity,
      }}
    >
      {source && (
        <img
          src={source}
          objectFit="fill"
          alt=""
          role="presentation"
          style={{ width: "100%", height: "100%", pointerEvents: "none" }}
        />
      )}
    </div>
  );
}
