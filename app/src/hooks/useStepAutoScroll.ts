import { useEffect, useRef, useState, useCallback } from "react";
import { useAutoScrollStore } from "@/stores/useAutoScrollStore";

function scrollInContainer(
  container: HTMLElement,
  el: HTMLElement,
  programmaticRef: React.MutableRefObject<boolean>,
) {
  const offset =
    el.getBoundingClientRect().top -
    container.getBoundingClientRect().top +
    container.scrollTop;

  programmaticRef.current = true;
  container.scrollTo({ top: offset, behavior: "smooth" });

  // Clear guard after scroll settles
  const clear = () => {
    programmaticRef.current = false;
  };
  if ("onscrollend" in container) {
    container.addEventListener("scrollend", clear, { once: true });
    // Fallback in case scrollend never fires
    setTimeout(clear, 2000);
  } else {
    setTimeout(clear, 1500);
  }
}

/**
 * Check if an element is visible within a scroll container.
 */
function isElementVisibleInContainer(el: HTMLElement, container: HTMLElement): boolean {
  const elRect = el.getBoundingClientRect();
  const containerRect = container.getBoundingClientRect();
  // Consider visible if at least part of the element is within the container viewport
  return (
    elRect.top < containerRect.bottom &&
    elRect.bottom > containerRect.top
  );
}

/**
 * Auto-scrolls to the currently running step.
 * Disables on manual scroll; shows a floating "go to active" button.
 * Resets when a new test run starts (running transitions from false→true).
 */
export function useStepAutoScroll(
  scrollContainerRef: React.RefObject<HTMLElement>,
  running: boolean,
  results: Record<string, { status?: string } | undefined>,
) {
  const enabled = useAutoScrollStore((s) => s.enabled);
  const [autoScrollActive, setAutoScrollActive] = useState(true);
  const [showGoTo, setShowGoTo] = useState(false);
  const prevRunningRef = useRef(false);
  const programmaticScrollRef = useRef(false);

  // Reset auto-scroll when a new test starts
  useEffect(() => {
    if (running && !prevRunningRef.current) {
      setAutoScrollActive(true);
      setShowGoTo(false);
    }
    prevRunningRef.current = running;
  }, [running]);

  // Find the current target step element
  const getTargetStepElement = useCallback((): HTMLElement | null => {
    const keys = Object.keys(results);
    const targetId =
      keys.find((id) => results[id]?.status === "running") ??
      keys.find((id) => results[id]?.status === "pending") ??
      keys.find((id) => !results[id]);
    if (!targetId) return null;
    return document.querySelector(`[data-step-id="${targetId}"]`) as HTMLElement | null;
  }, [results]);

  // Detect manual scroll → disable auto-scroll, but check visibility
  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container || !enabled) return;

    let scrollTimeout: ReturnType<typeof setTimeout>;
    const handleScroll = () => {
      if (programmaticScrollRef.current) return;
      if (!running || !autoScrollActive) {
        // Even when auto-scroll is already off, update showGoTo based on visibility
        if (!running) return;
        const el = getTargetStepElement();
        if (el && isElementVisibleInContainer(el, container)) {
          setShowGoTo(false);
        } else if (!autoScrollActive) {
          setShowGoTo(true);
        }
        return;
      }
      clearTimeout(scrollTimeout);
      scrollTimeout = setTimeout(() => {
        if (programmaticScrollRef.current) return;
        setAutoScrollActive(false);
        // Check if target is visible — if so, don't show the button
        const el = getTargetStepElement();
        if (el && isElementVisibleInContainer(el, container)) {
          setShowGoTo(false);
        } else {
          setShowGoTo(true);
        }
      }, 80);
    };

    container.addEventListener("scroll", handleScroll, { passive: true });
    return () => {
      container.removeEventListener("scroll", handleScroll);
      clearTimeout(scrollTimeout);
    };
  }, [scrollContainerRef, running, autoScrollActive, enabled, getTargetStepElement]);

  // Find running step and scroll to it
  useEffect(() => {
    if (!enabled || !autoScrollActive || !running) return;

    const runningStepId = Object.keys(results).find(
      (id) => results[id]?.status === "running",
    );
    if (!runningStepId) return;

    const el = document.querySelector(`[data-step-id="${runningStepId}"]`) as HTMLElement | null;
    const container = scrollContainerRef.current;
    if (!el || !container) return;

    scrollInContainer(container, el, programmaticScrollRef);
  }, [results, running, autoScrollActive, enabled, scrollContainerRef]);

  // Hide GoTo when test finishes
  useEffect(() => {
    if (!running) {
      setShowGoTo(false);
    }
  }, [running]);

  const goToRunningStep = useCallback(() => {
    const container = scrollContainerRef.current;
    if (!container) return;

    const el = getTargetStepElement();
    if (!el) return;

    // Re-enable auto-scroll BEFORE scrolling so the handler ignores events
    setAutoScrollActive(true);
    setShowGoTo(false);

    scrollInContainer(container, el, programmaticScrollRef);
  }, [getTargetStepElement, scrollContainerRef]);

  return {
    showGoToButton: enabled && showGoTo && running,
    goToRunningStep,
  };
}

/**
 * Check if any of the given step IDs are visible in the scroll container.
 */
export function useStepVisibility(
  scrollContainerRef: React.RefObject<HTMLElement>,
  stepIds: string[],
): boolean {
  const [anyVisible, setAnyVisible] = useState(false);

  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container || stepIds.length === 0) {
      setAnyVisible(false);
      return;
    }

    const check = () => {
      for (const id of stepIds) {
        const el = document.querySelector(`[data-step-id="${id}"]`) as HTMLElement | null;
        if (el && isElementVisibleInContainer(el, container)) {
          setAnyVisible(true);
          return;
        }
      }
      setAnyVisible(false);
    };

    check();
    container.addEventListener("scroll", check, { passive: true });
    return () => container.removeEventListener("scroll", check);
  }, [scrollContainerRef, stepIds]);

  return anyVisible;
}
