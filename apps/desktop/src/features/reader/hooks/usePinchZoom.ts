import { useEffect, useRef, type RefObject } from "react";

/**
 * Pinch to zoom on the page: a trackpad pinch (which arrives as Ctrl +
 * wheel on Windows, Linux and in Chromium, and as gesture events in
 * Safari's web view on macOS), Ctrl/⌘ + mouse wheel, and two fingers on a
 * touch screen. `onZoom` gets a factor (above 1: larger) and the point the
 * zoom is around, at most once a frame.
 */
export function usePinchZoom(
  host: RefObject<HTMLElement | null>,
  enabled: boolean,
  onZoom: (factor: number, x: number, y: number) => void,
) {
  const cb = useRef(onZoom);
  useEffect(() => {
    cb.current = onZoom;
  });

  useEffect(() => {
    const el = host.current;
    if (!el || !enabled) return;
    let pending = 1;
    let at = { x: 0, y: 0 };
    let frame = 0;
    const push = (factor: number, x: number, y: number) => {
      if (!Number.isFinite(factor) || factor <= 0) return;
      pending *= factor;
      at = { x, y };
      if (frame) return;
      frame = requestAnimationFrame(() => {
        frame = 0;
        const f = pending;
        pending = 1;
        if (Math.abs(f - 1) > 0.002) cb.current(f, at.x, at.y);
      });
    };

    // Trackpad pinch (Ctrl + wheel) and Ctrl/⌘ + mouse wheel.
    const onWheel = (e: WheelEvent) => {
      if (!e.ctrlKey && !e.metaKey) return;
      e.preventDefault();
      const dy = e.deltaMode === 1 ? e.deltaY * 16 : e.deltaY;
      // Pinches send small steps, wheels large ones; both feel even.
      push(Math.exp(-Math.max(-25, Math.min(25, dy)) * 0.005), e.clientX, e.clientY);
    };

    // Safari's web view (macOS): gesture events with a running scale.
    let lastScale = 1;
    type Gesture = Event & { scale: number; clientX: number; clientY: number };
    const onGestureStart = (e: Event) => {
      e.preventDefault();
      lastScale = 1;
    };
    const onGestureChange = (e: Event) => {
      e.preventDefault();
      const g = e as Gesture;
      push(g.scale / lastScale, g.clientX, g.clientY);
      lastScale = g.scale;
    };

    // Two fingers on a touch screen.
    let lastDist = 0;
    const dist = (t: TouchList) =>
      Math.hypot(t[0]!.clientX - t[1]!.clientX, t[0]!.clientY - t[1]!.clientY);
    const onTouchStart = (e: TouchEvent) => {
      if (e.touches.length === 2) lastDist = dist(e.touches);
    };
    const onTouchMove = (e: TouchEvent) => {
      if (e.touches.length !== 2 || !lastDist) return;
      e.preventDefault();
      const d = dist(e.touches);
      const x = (e.touches[0]!.clientX + e.touches[1]!.clientX) / 2;
      const y = (e.touches[0]!.clientY + e.touches[1]!.clientY) / 2;
      push(d / lastDist, x, y);
      lastDist = d;
    };
    const onTouchEnd = (e: TouchEvent) => {
      if (e.touches.length < 2) lastDist = 0;
    };

    el.addEventListener("wheel", onWheel, { passive: false });
    el.addEventListener("gesturestart", onGestureStart);
    el.addEventListener("gesturechange", onGestureChange);
    el.addEventListener("touchstart", onTouchStart, { passive: true });
    el.addEventListener("touchmove", onTouchMove, { passive: false });
    el.addEventListener("touchend", onTouchEnd);
    el.addEventListener("touchcancel", onTouchEnd);
    return () => {
      cancelAnimationFrame(frame);
      el.removeEventListener("wheel", onWheel);
      el.removeEventListener("gesturestart", onGestureStart);
      el.removeEventListener("gesturechange", onGestureChange);
      el.removeEventListener("touchstart", onTouchStart);
      el.removeEventListener("touchmove", onTouchMove);
      el.removeEventListener("touchend", onTouchEnd);
      el.removeEventListener("touchcancel", onTouchEnd);
    };
  }, [host, enabled]);
}
