/** Scrolls the panel body so `element` is at its top (with the body's 12 px
 * padding). The body's `scrollTop` is set, not `scrollIntoView`, so no other
 * ancestor can ever scroll (UX review B2). */
export function scrollBodyTo(element: HTMLElement | null) {
  const body = element?.closest<HTMLElement>(".properties-panel-body");
  if (element && body) {
    body.scrollTop = Math.max(0, element.offsetTop - 12);
  }
}
