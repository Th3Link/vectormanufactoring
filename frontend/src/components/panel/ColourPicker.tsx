import { useEffect, useRef, useState } from "react";
import type { KeyboardEvent, PointerEvent } from "react";

import type { HsvTriple } from "@/hooks/useStylePanel";

const WIDTH = 244;
const AREA_HEIGHT = 96;
const HUE_HEIGHT = 12;

interface Hsv {
  /** Degrees, 0 to 360. */
  h: number;
  /** 0 to 1. */
  s: number;
  /** 0 to 1. */
  v: number;
}

interface ColourPickerProps {
  /** "Stroke", "Fill" or "Background": prefixes the accessible names. */
  name: string;
  /** Packed `0xRRGGBB` of the shared colour. */
  rgb: number;
  /** The edited objects differ in colour: no thumb. */
  mixed: boolean;
  /** The hue, saturation and value of a packed colour (Rust's rounding). */
  hsvOf: (rgb: number) => HsvTriple;
  /** The packed colour of a hue, saturation and value (Rust's rounding). */
  rgbOf: (hue: number, saturation: number, value: number) => number;
  /** A tick of a drag: shown on the canvas, nothing written. */
  onPreview: (hue: number, saturation: number, value: number) => void;
}

const clamp = (value: number, low: number, high: number) => Math.min(high, Math.max(low, value));

/** Paints `draw(x, y)` (a packed colour) for every device pixel of a canvas. */
function paint(
  canvas: HTMLCanvasElement,
  width: number,
  height: number,
  colourAt: (x: number, y: number) => [number, number, number],
) {
  const ratio = window.devicePixelRatio || 1;
  canvas.width = Math.round(width * ratio);
  canvas.height = Math.round(height * ratio);
  const context = canvas.getContext("2d");
  if (!context) {
    return;
  }
  const image = context.createImageData(canvas.width, canvas.height);
  for (let py = 0; py < canvas.height; py += 1) {
    for (let px = 0; px < canvas.width; px += 1) {
      const [r, g, b] = colourAt(px / Math.max(1, canvas.width - 1), py / Math.max(1, canvas.height - 1));
      const at = (py * canvas.width + px) * 4;
      image.data[at] = r;
      image.data[at + 1] = g;
      image.data[at + 2] = b;
      image.data[at + 3] = 255;
    }
  }
  context.putImageData(image, 0, 0);
}

const channels = (rgb: number): [number, number, number] => [
  (rgb >> 16) & 0xff,
  (rgb >> 8) & 0xff,
  rgb & 0xff,
];

/** A thumb: a 2 px ring inside a 1 px casing, so it reads on every colour. */
function Thumb({ size, left, top }: { size: number; left: number; top: number }) {
  return (
    <span
      aria-hidden
      className="pointer-events-none absolute rounded-full border-2 border-[var(--picker-thumb-ring)]"
      style={{
        width: size,
        height: size,
        left: left - size / 2,
        top: top - size / 2,
        boxShadow: "0 0 0 1px var(--picker-thumb-casing), inset 0 0 0 1px var(--picker-thumb-casing)",
      }}
    />
  );
}

/**
 * The inline colour picker (`specs/0017-style-panel-rework` criteria 17 to 21):
 * a saturation and value area and a hue slider, always visible under the Color
 * row, no popup. It keeps its own hue, saturation and value and re-derives them
 * from the stored colour only when the colour changed from outside it, so the
 * hue does not jump while a drag passes through a grey (criterion 20). The
 * rounding to 8-bit RGB, and the hue of a colour, are Rust's (`colour_hsv`);
 * a drag previews per frame and commits once on release
 * (`usePreviewGesture`). The alpha is not in the picker: the Opacity row owns it.
 */
export function ColourPicker({
  name,
  rgb,
  mixed,
  hsvOf,
  rgbOf,
  onPreview,
}: ColourPickerProps) {
  const [hsv, setHsv] = useState<Hsv>({ h: 0, s: 1, v: 1 });
  // The colour the picker last sent, and the one it last read from outside.
  const [seen, setSeen] = useState<number | null>(null);
  const [own, setOwn] = useState<number | null>(null);
  const areaCanvas = useRef<HTMLCanvasElement>(null);
  const hueCanvas = useRef<HTMLCanvasElement>(null);

  if (!mixed && seen !== rgb && own !== rgb) {
    const [hue, saturation, value] = hsvOf(rgb);
    setSeen(rgb);
    setHsv((previous) => ({ h: hue < 0 ? previous.h : hue, s: saturation, v: value }));
  } else if (!mixed && seen !== rgb) {
    setSeen(rgb);
  }

  useEffect(() => {
    const canvas = areaCanvas.current;
    if (canvas) {
      const [hr, hg, hb] = channels(rgbOf(hsv.h, 1, 1));
      paint(canvas, WIDTH, AREA_HEIGHT, (x, y) => {
        const light = 1 - y;
        return [
          Math.round((255 + (hr - 255) * x) * light),
          Math.round((255 + (hg - 255) * x) * light),
          Math.round((255 + (hb - 255) * x) * light),
        ];
      });
    }
  }, [hsv.h, rgbOf]);
  useEffect(() => {
    const canvas = hueCanvas.current;
    if (canvas) {
      paint(canvas, WIDTH, HUE_HEIGHT, (x) => channels(rgbOf(360 * x, 1, 1)));
    }
  }, [rgbOf]);

  const apply = (next: Hsv) => {
    const clamped = { h: clamp(next.h, 0, 360), s: clamp(next.s, 0, 1), v: clamp(next.v, 0, 1) };
    setOwn(rgbOf(clamped.h, clamped.s, clamped.v));
    setHsv(clamped);
    onPreview(clamped.h, clamped.s, clamped.v);
  };

  const dragArea = (event: PointerEvent<HTMLDivElement>) => {
    const box = event.currentTarget.getBoundingClientRect();
    apply({
      h: hsv.h,
      s: (event.clientX - box.left) / box.width,
      v: 1 - (event.clientY - box.top) / box.height,
    });
  };
  const dragHue = (event: PointerEvent<HTMLDivElement>) => {
    const box = event.currentTarget.getBoundingClientRect();
    apply({ ...hsv, h: (360 * (event.clientX - box.left)) / box.width });
  };

  const press = (move: (event: PointerEvent<HTMLDivElement>) => void) => ({
    onPointerDown: (event: PointerEvent<HTMLDivElement>) => {
      if (event.button !== 0) {
        return;
      }
      event.currentTarget.setPointerCapture(event.pointerId);
      event.currentTarget.focus({ preventScroll: true });
      move(event);
    },
    onPointerMove: (event: PointerEvent<HTMLDivElement>) => {
      if (event.currentTarget.hasPointerCapture(event.pointerId)) {
        move(event);
      }
    },
  });

  const step = (event: KeyboardEvent) => (event.shiftKey ? 0.1 : 0.01);

  const areaKeys = (event: KeyboardEvent<HTMLDivElement>) => {
    const by = step(event);
    const moves: Record<string, Partial<Hsv>> = {
      ArrowLeft: { s: hsv.s - by },
      ArrowRight: { s: hsv.s + by },
      ArrowUp: { v: hsv.v + by },
      ArrowDown: { v: hsv.v - by },
    };
    const change = moves[event.key];
    if (change) {
      event.preventDefault();
      apply({ ...hsv, ...change });
    }
  };
  const hueKeys = (event: KeyboardEvent<HTMLDivElement>) => {
    const by = 360 * step(event);
    const delta =
      event.key === "ArrowLeft" || event.key === "ArrowDown"
        ? -by
        : event.key === "ArrowRight" || event.key === "ArrowUp"
          ? by
          : 0;
    if (delta !== 0) {
      event.preventDefault();
      apply({ ...hsv, h: hsv.h + delta });
    }
  };

  const slider =
    "relative cursor-crosshair outline-none focus-visible:ring-2 focus-visible:ring-[var(--editor-accent)] focus-visible:ring-offset-1 focus-visible:ring-offset-[var(--toolbar-bg)] rounded-[5px]";
  const percentOf = (value: number) => Math.round(value * 100);

  return (
    <div className="flex flex-col gap-2" style={{ width: WIDTH }}>
      <div
        role="slider"
        tabIndex={0}
        aria-label={`${name} saturation and value`}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={mixed ? undefined : percentOf(hsv.v)}
        aria-valuetext={
          mixed
            ? "Mixed"
            : `Saturation ${percentOf(hsv.s)} %, value ${percentOf(hsv.v)} %`
        }
        className={slider}
        style={{ width: WIDTH, height: AREA_HEIGHT, touchAction: "none" }}
        onKeyDown={areaKeys}
        {...press(dragArea)}
      >
        <canvas
          ref={areaCanvas}
          aria-hidden
          className="block rounded-[5px]"
          style={{ width: WIDTH, height: AREA_HEIGHT }}
        />
        {!mixed && <Thumb size={14} left={hsv.s * WIDTH} top={(1 - hsv.v) * AREA_HEIGHT} />}
      </div>
      <div
        role="slider"
        tabIndex={0}
        aria-label={`${name} hue`}
        aria-valuemin={0}
        aria-valuemax={360}
        aria-valuenow={mixed ? undefined : Math.round(hsv.h)}
        aria-valuetext={mixed ? "Mixed" : `Hue ${Math.round(hsv.h)} degrees`}
        className={slider}
        style={{ width: WIDTH, height: HUE_HEIGHT, touchAction: "none" }}
        onKeyDown={hueKeys}
        {...press(dragHue)}
      >
        <canvas
          ref={hueCanvas}
          aria-hidden
          className="block rounded-[6px]"
          style={{ width: WIDTH, height: HUE_HEIGHT }}
        />
        {!mixed && <Thumb size={16} left={(hsv.h / 360) * WIDTH} top={HUE_HEIGHT / 2} />}
      </div>
    </div>
  );
}
