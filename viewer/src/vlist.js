// Virtualized list: only the visible rows exist in the DOM, so 17k options
// render instantly.

class VirtualList {
  constructor(container, renderRow, rowHeight = 26) {
    this.container = container;
    this.renderRow = renderRow;
    this.rowHeight = rowHeight;
    this.items = [];
    this.selected = -1;
    this.spacer = h("div", { class: "spacer" });
    this.emptyText = "";
    container.replaceChildren(this.spacer);
    container.addEventListener("scroll", () => this.draw());
    new ResizeObserver(() => this.draw()).observe(container);
  }

  setItems(items, emptyText = "Nothing to show.") {
    this.items = items;
    this.emptyText = emptyText;
    this.container.scrollTop = 0;
    this.draw();
  }

  setSelected(i) {
    this.selected = i;
    this.draw();
  }

  // Scroll row i into view (with some context above it) if it is not visible.
  reveal(i) {
    const top = i * this.rowHeight;
    const c = this.container;
    if (top < c.scrollTop || top + this.rowHeight > c.scrollTop + c.clientHeight) c.scrollTop = Math.max(0, top - 2 * this.rowHeight);
    this.draw();
  }

  draw() {
    const n = this.items.length;
    if (n === 0) {
      this.spacer.style.height = "auto";
      this.spacer.replaceChildren(h("div", { class: "empty" }, this.emptyText));
      return;
    }
    this.spacer.style.height = `${n * this.rowHeight}px`;
    const top = this.container.scrollTop;
    const height = this.container.clientHeight || 400;
    const first = Math.max(0, Math.floor(top / this.rowHeight) - 5);
    const last = Math.min(n, Math.ceil((top + height) / this.rowHeight) + 5);
    const rows = [];
    for (let i = first; i < last; i++) {
      const row = this.renderRow(this.items[i], i);
      row.classList.add("row");
      if (i === this.selected) row.classList.add("sel");
      row.style.top = `${i * this.rowHeight}px`;
      rows.push(row);
    }
    this.spacer.replaceChildren(...rows);
  }
}
