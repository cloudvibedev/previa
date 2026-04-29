import { useEffect, useRef, useCallback, useState, type ReactNode, type CSSProperties } from "react";
import { StepFlowList } from "@/components/StepFlowList";

/* ── Types ── */

export interface StepFlowGraphItem {
  key: string;
  content: ReactNode;
  status?: "pending" | "running" | "success" | "error";
}

interface StepFlowGraphProps {
  items: StepFlowGraphItem[];
  className?: string;
  itemHeight?: number;
}

/* ── Breakpoints ── */
const COL_MIN_WIDTH = 310; // px per column
const MAX_COLS = 4;

function getGridCoords(index: number, cols: number) {
  const row = Math.floor(index / cols);
  const col = index % cols;
  return { row, col };
}

/* ── Arrow color by status ── */
function getArrowColor(_status?: string): string {
  return "hsl(var(--foreground))";
}

/* ── SVG Arrow Overlay ── */
function ArrowOverlay({
  containerRef,
  cardRefs,
  items,
  cols,
}: {
  containerRef: React.RefObject<HTMLDivElement>;
  cardRefs: React.RefObject<Map<string, HTMLDivElement>>;
  items: StepFlowGraphItem[];
  cols: number;
}) {
  const svgRef = useRef<SVGSVGElement>(null);

  const drawArrows = useCallback(() => {
    const svg = svgRef.current;
    const container = containerRef.current;
    const cards = cardRefs.current;
    if (!svg || !container || !cards) return;

    const containerRect = container.getBoundingClientRect();
    svg.setAttribute("width", String(container.scrollWidth));
    svg.setAttribute("height", String(container.scrollHeight));

    while (svg.firstChild) svg.removeChild(svg.firstChild);

    const defs = document.createElementNS("http://www.w3.org/2000/svg", "defs");
    svg.appendChild(defs);

    for (let i = 0; i < items.length - 1; i++) {
      const sourceEl = cards.get(items[i].key);
      const targetEl = cards.get(items[i + 1].key);
      if (!sourceEl || !targetEl) continue;

      const sourceRect = sourceEl.getBoundingClientRect();
      const targetRect = targetEl.getBoundingClientRect();
      const color = getArrowColor(items[i].status);

      const sourceCoords = getGridCoords(i, cols);
      const targetCoords = getGridCoords(i + 1, cols);

      const PAD = 8;
      let x1: number, y1: number, x2: number, y2: number;

      if (sourceCoords.row === targetCoords.row) {
        x1 = sourceRect.right - containerRect.left + PAD;
        y1 = sourceRect.top + sourceRect.height / 2 - containerRect.top;
        x2 = targetRect.left - containerRect.left - PAD;
        y2 = targetRect.top + targetRect.height / 2 - containerRect.top;
      } else {
        x1 = sourceRect.left + sourceRect.width / 2 - containerRect.left;
        y1 = sourceRect.bottom - containerRect.top + PAD;
        x2 = targetRect.left + targetRect.width / 2 - containerRect.left;
        y2 = targetRect.top - containerRect.top - PAD;
      }

      const markerId = `arrow-${i}`;
      const marker = document.createElementNS("http://www.w3.org/2000/svg", "marker");
      marker.setAttribute("id", markerId);
      marker.setAttribute("markerWidth", "10");
      marker.setAttribute("markerHeight", "10");
      marker.setAttribute("refX", "9");
      marker.setAttribute("refY", "5");
      marker.setAttribute("orient", "auto");
      const polygon = document.createElementNS("http://www.w3.org/2000/svg", "polygon");
      polygon.setAttribute("points", "0 1, 10 5, 0 9");
      polygon.setAttribute("fill", color);
      polygon.setAttribute("opacity", "0.85");
      marker.appendChild(polygon);
      defs.appendChild(marker);

      const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
      const midX = (x1 + x2) / 2;

      let d: string;
      if (sourceCoords.row === targetCoords.row) {
        d = `M ${x1} ${y1} C ${midX} ${y1}, ${midX} ${y2}, ${x2} ${y2}`;
      } else {
        const r = 16;
        const midY = (y1 + y2) / 2;
        if (x1 > x2) {
          d = `M ${x1} ${y1} L ${x1} ${midY - r} Q ${x1} ${midY}, ${x1 - r} ${midY} L ${x2 + r} ${midY} Q ${x2} ${midY}, ${x2} ${midY + r} L ${x2} ${y2}`;
        } else {
          d = `M ${x1} ${y1} L ${x1} ${midY - r} Q ${x1} ${midY}, ${x1 + r} ${midY} L ${x2 - r} ${midY} Q ${x2} ${midY}, ${x2} ${midY + r} L ${x2} ${y2}`;
        }
      }

      path.setAttribute("d", d);
      path.setAttribute("fill", "none");
      path.setAttribute("stroke", color);
      path.setAttribute("stroke-width", "1.5");
      path.setAttribute("stroke-opacity", "0.6");
      path.setAttribute("stroke-linecap", "round");
      path.setAttribute("marker-end", `url(#${markerId})`);

      const sourceStatus = items[i].status;
      const targetStatus = items[i + 1]?.status;
      const isActiveArrow = sourceStatus === "running";
      const isPendingArrow = !sourceStatus || sourceStatus === "pending" || (!targetStatus || targetStatus === "pending");

      if (isActiveArrow || isPendingArrow) {
        path.setAttribute("stroke-dasharray", isActiveArrow ? "8 5" : "4 6");
        path.setAttribute("stroke-opacity", isActiveArrow ? "0.9" : "0.35");
        const animate = document.createElementNS("http://www.w3.org/2000/svg", "animate");
        animate.setAttribute("attributeName", "stroke-dashoffset");
        animate.setAttribute("from", "26");
        animate.setAttribute("to", "0");
        animate.setAttribute("dur", isActiveArrow ? "0.6s" : "1.5s");
        animate.setAttribute("repeatCount", "indefinite");
        path.appendChild(animate);
      }

      svg.appendChild(path);
    }
  }, [containerRef, cardRefs, items, cols]);

  useEffect(() => {
    drawArrows();

    const container = containerRef.current;
    if (!container) return;

    const observer = new ResizeObserver(() => drawArrows());
    observer.observe(container);

    const timer = setTimeout(drawArrows, 100);

    return () => {
      observer.disconnect();
      clearTimeout(timer);
    };
  }, [drawArrows]);

  return (
    <svg
      ref={svgRef}
      className="absolute inset-0 pointer-events-none"
      style={{ overflow: "visible", zIndex: 1 }}
    />
  );
}

/* ── Main Component ── */
export function StepFlowGraph({ items, className, itemHeight = 95 }: StepFlowGraphProps) {
  const containerRef = useRef<HTMLDivElement>(null!);
  const cardRefs = useRef<Map<string, HTMLDivElement>>(new Map());
  const [cols, setCols] = useState(MAX_COLS);

  // Measure container width and compute columns
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const update = () => {
      const w = container.clientWidth;
      const c = Math.min(MAX_COLS, Math.max(1, Math.floor(w / COL_MIN_WIDTH)));
      setCols(c);
    };

    update();
    const observer = new ResizeObserver(update);
    observer.observe(container);
    return () => observer.disconnect();
  }, []);

  const gridItems = items.map((item, index) => {
    const { row, col } = getGridCoords(index, cols);
    const gridStyle: CSSProperties = {
      gridRow: row + 1,
      gridColumn: col + 1,
      position: "relative" as const,
      zIndex: 2,
    };

    return (
      <div
        key={item.key}
        ref={(el) => {
          if (el) cardRefs.current.set(item.key, el);
          else cardRefs.current.delete(item.key);
        }}
        style={{ ...gridStyle, height: itemHeight }}
      >
        {item.content}
      </div>
    );
  });

  const totalRows = items.length > 0 ? Math.floor((items.length - 1) / cols) + 1 : 0;

  return (
    <div
      ref={containerRef}
      className={`relative w-full ${className ?? ""}`}
      style={{ position: "relative" }}
    >
      {cols <= 1 ? (
        <StepFlowList items={items} />
      ) : (
        <div
          className="grid gap-x-8 gap-y-20"
          style={{
            gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))`,
            gridTemplateRows: `repeat(${totalRows}, auto)`,
          }}
        >
          {gridItems}
        </div>
      )}
      {cols > 1 && (
        <ArrowOverlay containerRef={containerRef} cardRefs={cardRefs} items={items} cols={cols} />
      )}
    </div>
  );
}
