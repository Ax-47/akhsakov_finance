// Technical stock chart. Rust computes every series (candles, overlays,
// bands, indicator panes) and keeps them moving with each streamed trade;
// this file draws them with ECharts (loaded through GrowthChart.ready) and
// owns what is purely on screen: the layout of the panes, the time axis,
// the readout that follows the crosshair, zoom and pan, the keyboard, and
// the lines the viewer places (levels, trend lines, Fibonacci retracements,
// a measure), which Rust saves.
//
//   StockChart.init(id, cfg)     draws from scratch
//   StockChart.patch(id, p)      moves the last candle or adds the next one
//   StockChart.command(id, name) "reset" | "latest" | "in" | "out"
//   StockChart.png(id)           the chart as a PNG data URL
(function () {
  var SC = (window.StockChart = window.StockChart || {});
  SC.latest = SC.latest || {};
  SC.charts = SC.charts || {};
  // Per chart: saved drawings, the tool in use, and where placed points go.
  SC.drawings = SC.drawings || {};
  SC.tools = SC.tools || {};
  SC.listeners = SC.listeners || {};
  SC.full = SC.full || {};

  var FIB_LEVELS = [0, 0.236, 0.382, 0.5, 0.618, 0.786, 1];
  // Room around the plots, in pixels: the price scale on the right, the
  // time labels and the zoom bar underneath.
  var EDGE = { left: 8, right: 64, top: 6, bottom: 44, gap: 8 };
  var DAY = 86400;

  // Saved drawings: [{ id, kind: "Level" | "Trend" | "Fibonacci", points: [{ ts, price }] }].
  SC.setDrawings = function (id, list) {
    SC.drawings[id] = list || [];
    renderDrawings(id);
  };

  // The tool clicks place: "level", "trend", "fibonacci", "measure", "erase" or null.
  SC.setTool = function (id, tool) {
    SC.tools[id] = { tool: tool || null, first: null, preview: null, measure: null };
    var chart = live(id);
    if (chart) chart.getZr().setCursorStyle(tool ? "crosshair" : "default");
    renderDrawings(id);
  };

  // Placed drawings, erase clicks and keys go to `cb`: { add, points },
  // { erase }, { cancel } (Esc while drawing) or { exit } (Esc in full screen).
  SC.listen = function (id, cb) {
    SC.listeners[id] = cb;
  };

  // In full screen the wheel zooms without Ctrl (there is no page to
  // scroll), and Esc anywhere leaves it (or drops the tool in use first).
  SC.setFull = function (id, on) {
    SC.full[id] = !!on;
    SC.escape = SC.escape || {};
    if (SC.escape[id]) {
      document.removeEventListener("keydown", SC.escape[id], true);
      delete SC.escape[id];
    }
    if (on) {
      SC.escape[id] = function (e) {
        var cb = SC.listeners[id];
        if (e.key !== "Escape" || !cb) return;
        var t = SC.tools[id];
        e.stopPropagation();
        cb(t && t.tool ? { cancel: true } : { exit: true });
      };
      document.addEventListener("keydown", SC.escape[id], true);
    }
    var chart = live(id);
    if (chart) chart.setOption({ dataZoom: [{ id: "inside", zoomOnMouseWheel: on ? true : "ctrl" }] });
  };

  // Frees a chart whose component unmounted.
  SC.dispose = function (id) {
    SC.setFull(id, false);
    ["latest", "drawings", "tools", "listeners", "full"].forEach(function (k) { delete SC[k][id]; });
    var chart = SC.charts[id];
    delete SC.charts[id];
    if (!chart) return;
    if (chart.__unwatch) chart.__unwatch();
    if (!chart.isDisposed()) chart.dispose();
  };

  SC.init = function (id, cfg) {
    SC.latest[id] = cfg;
    var ready = window.GrowthChart && window.GrowthChart.ready
      ? window.GrowthChart.ready()
      : Promise.reject(new Error("growth_chart.js not loaded"));
    ready.then(function () {
      if (SC.latest[id] !== cfg) return; // superseded
      var el = document.getElementById(id);
      if (!el) return;
      draw(el, id, cfg);
    }).catch(function (e) { console.error("[StockChart]", e); });
  };

  SC.png = function (id) {
    var chart = live(id);
    if (!chart) return "";
    return chart.getDataURL({ type: "png", pixelRatio: 2, backgroundColor: chart.__data.col.base });
  };

  SC.command = function (id, name) {
    var chart = live(id);
    var d = chart && chart.__data;
    if (!d) return;
    var z = zoomOf(chart, d);
    var span = z.to - z.from;
    var last = d.times.length - 1;
    if (name === "reset") setZoom(chart, d, d.cfg.zoom.from, last);
    else if (name === "latest") setZoom(chart, d, last - span, last);
    else if (name === "in" || name === "out") {
      // About the right edge when it is the newest candle, else the middle.
      var next = Math.max(8, Math.round(name === "in" ? span * 0.7 : span / 0.7 + 1));
      if (z.to >= last) setZoom(chart, d, last - next, last);
      else { var mid = (z.from + z.to) / 2; setZoom(chart, d, Math.round(mid - next / 2), Math.round(mid + next / 2)); }
    } else if (name === "left" || name === "right") {
      var by = Math.max(1, Math.round(span * 0.15)) * (name === "left" ? -1 : 1);
      setZoom(chart, d, z.from + by, z.to + by);
    }
  };

  // Moves the chart with a streamed trade. `p`: { time, added, ohlc, shown,
  // volume, values: [[series id, value]] }. Rust has already worked out the
  // candle and every indicator's newest value.
  SC.patch = function (id, p) {
    var chart = live(id);
    var d = chart && chart.__data;
    if (!d || !d.times.length) return;
    var n = d.times.length - 1;
    var pinned = zoomOf(chart, d).to >= n;
    if (p.added) {
      if (p.time <= d.times[n]) return;
      d.times.push(p.time);
      d.cats.push(String(n + 1));
      d.ohlc.push(p.ohlc);
      d.shown.push(p.shown || p.ohlc);
      d.volume.push(p.volume || 0);
      Object.keys(d.lines).forEach(function (k) { d.lines[k].push(null); });
      d.local = null;
      n += 1;
    } else {
      // A candle this chart does not end on: the next full draw has it.
      if (p.time !== d.times[n]) return;
      d.ohlc[n] = p.ohlc;
      d.shown[n] = p.shown || p.ohlc;
      d.volume[n] = p.volume || 0;
    }
    (p.values || []).forEach(function (pair) {
      var line = d.lines[pair[0]];
      if (line) line[n] = pair[1];
    });
    var option = { series: liveSeries(d) };
    if (p.added) {
      option.xAxis = d.grids.map(function () { return { data: d.cats }; });
      // Looking at the newest candle: keep looking at it.
      if (pinned) {
        var z = zoomOf(chart, d);
        option.dataZoom = [{ id: "inside", startValue: z.from + 1, endValue: n }, { id: "slider", startValue: z.from + 1, endValue: n }];
      }
    }
    chart.setOption(option);
    if (d.hover === null) renderLegend(d);
  };

  function live(id) {
    var chart = SC.charts[id];
    return chart && !chart.isDisposed() && chart.__data ? chart : null;
  }

  // ── Numbers and times ────────────────────────────────────────────────────

  function fmtPrice(v) {
    var a = Math.abs(v);
    var digits = a >= 1 || a === 0 ? 2 : 4;
    return Number(v).toLocaleString(undefined, { minimumFractionDigits: digits, maximumFractionDigits: digits });
  }
  function fmtValue(v) {
    var a = Math.abs(v);
    if (a >= 1e5) return compact(v);
    return Number(v).toLocaleString(undefined, { maximumFractionDigits: a >= 100 ? 1 : 2 });
  }
  function compact(v) {
    var a = Math.abs(v);
    if (a >= 1e12) return (v / 1e12).toFixed(2) + "T";
    if (a >= 1e9) return (v / 1e9).toFixed(2) + "B";
    if (a >= 1e6) return (v / 1e6).toFixed(2) + "M";
    if (a >= 1e3) return (v / 1e3).toFixed(1) + "K";
    return String(Math.round(v * 100) / 100);
  }
  // An axis label of at most three significant digits, e.g. -300M or 1.25B.
  function shortNumber(v) {
    var a = Math.abs(v);
    var units = [[1e9, "B"], [1e6, "M"], [1e3, "K"]];
    for (var i = 0; i < units.length; i++) {
      if (a >= units[i][0]) return Number((v / units[i][0]).toPrecision(3)) + units[i][1];
    }
    return String(Number(v.toPrecision(3)));
  }

  // The candle a time falls in: the last one starting at or before it.
  function indexAt(times, ts) {
    var lo = 0, hi = times.length - 1;
    if (hi < 0 || ts < times[0]) return -1;
    while (lo < hi) {
      var mid = (lo + hi + 1) >> 1;
      if (times[mid] <= ts) lo = mid; else hi = mid - 1;
    }
    return lo;
  }

  // Calendar fields of every candle, worked out once per draw: minute
  // candles in the viewer's own time, daily ones by the date they carry.
  function localOf(d) {
    if (d.local && d.local.length === d.times.length) return d.local;
    d.local = d.times.map(function (t) {
      var x = new Date(t * 1000);
      // `dn` counts days, for telling weeks apart.
      return d.intraday
        ? { y: x.getFullYear(), m: x.getMonth(), day: x.getDate(), min: x.getHours() * 60 + x.getMinutes(), dn: Math.floor(Date.UTC(x.getFullYear(), x.getMonth(), x.getDate()) / 864e5) }
        : { y: x.getUTCFullYear(), m: x.getUTCMonth(), day: x.getUTCDate(), min: 0, dn: Math.floor(t / DAY) };
    });
    d.bounds = {};
    return d.local;
  }

  // Steps the time axis can count in, finest first. `key` says which
  // step a candle belongs to; a label goes where it changes.
  var STEPS = [
    { id: "5m", intraday: true, key: function (c) { return c.y * 1e8 + c.m * 1e6 + c.day * 1e4 + Math.floor(c.min / 5); } },
    { id: "15m", intraday: true, key: function (c) { return c.y * 1e8 + c.m * 1e6 + c.day * 1e4 + Math.floor(c.min / 15); } },
    { id: "30m", intraday: true, key: function (c) { return c.y * 1e8 + c.m * 1e6 + c.day * 1e4 + Math.floor(c.min / 30); } },
    { id: "1h", intraday: true, key: function (c) { return c.y * 1e8 + c.m * 1e6 + c.day * 1e4 + Math.floor(c.min / 60); } },
    { id: "2h", intraday: true, key: function (c) { return c.y * 1e8 + c.m * 1e6 + c.day * 1e4 + Math.floor(c.min / 120); } },
    { id: "day", key: function (c) { return c.y * 1e4 + c.m * 100 + c.day; } },
    // Monday to Sunday (day 0 was a Thursday).
    { id: "week", key: function (c) { return Math.floor((c.dn + 3) / 7); } },
    { id: "month", key: function (c) { return c.y * 12 + c.m; } },
    { id: "quarter", key: function (c) { return c.y * 4 + Math.floor(c.m / 3); } },
    { id: "year", key: function (c) { return c.y; } },
    { id: "5y", key: function (c) { return Math.floor(c.y / 5); } },
  ];

  // Where each step starts, over all the candles: index → true.
  function boundsOf(d, step) {
    var local = localOf(d);
    if (d.bounds[step.id]) return d.bounds[step.id];
    var set = {}, count = 0, prev = null;
    for (var i = 0; i < local.length; i++) {
      var k = step.key(local[i]);
      if (prev !== null && k !== prev) { set[i] = true; count++; }
      prev = k;
    }
    return (d.bounds[step.id] = { set: set, count: count });
  }

  // The finest step whose labels still fit: `visible` candles across
  // `width` pixels, at about 80 pixels a label.
  function stepFor(d, visible, width) {
    var room = Math.max(2, Math.floor(width / 80));
    var total = d.times.length;
    for (var i = 0; i < STEPS.length; i++) {
      var s = STEPS[i];
      if (s.intraday && !d.intraday) continue;
      if (!d.intraday && s.id === "day" && d.step > DAY) continue;
      var b = boundsOf(d, s);
      if (b.count / total * visible <= room) return s;
    }
    return STEPS[STEPS.length - 1];
  }

  var MONTHS = null;
  function monthName(m) {
    if (!MONTHS) {
      MONTHS = [];
      for (var i = 0; i < 12; i++) MONTHS.push(new Date(Date.UTC(2020, i, 15)).toLocaleDateString(undefined, { month: "short", timeZone: "UTC" }));
    }
    return MONTHS[m];
  }
  function pad(n) { return (n < 10 ? "0" : "") + n; }

  // What the axis says where a step starts at candle `i`.
  function tickLabel(d, i) {
    var local = localOf(d);
    var c = local[i], p = local[i - 1];
    if (!c) return "";
    if (p && c.y !== p.y) return d.intraday ? c.day + " " + monthName(c.m) : String(c.y);
    if (p && c.m !== p.m) return d.intraday ? c.day + " " + monthName(c.m) : monthName(c.m);
    if (p && c.day !== p.day) return d.intraday ? c.day + " " + monthName(c.m) : String(c.day);
    return pad(Math.floor(c.min / 60)) + ":" + pad(c.min % 60);
  }

  // The crosshair's own label: the candle's whole date, and time of day.
  function fullLabel(d, i) {
    var c = localOf(d)[i];
    if (!c) return "";
    var date = c.day + " " + monthName(c.m) + " " + c.y;
    return d.intraday ? date + " " + pad(Math.floor(c.min / 60)) + ":" + pad(c.min % 60) : date;
  }

  // ── Layout ───────────────────────────────────────────────────────────────

  // The plots, top to bottom, in pixels: price, volume over the foot of
  // the price plot, then one pane per indicator.
  function layoutOf(el, panes) {
    var H = el.clientHeight || 460;
    var room = H - EDGE.top - EDGE.bottom;
    var pane = Math.round(Math.max(64, Math.min(150, H * 0.17)));
    var price = room - panes * (pane + EDGE.gap);
    if (panes && price < 170) {
      pane = Math.max(48, Math.floor((room - 170) / panes) - EDGE.gap);
      price = room - panes * (pane + EDGE.gap);
    }
    price = Math.max(80, price);
    var grids = [
      { left: EDGE.left, right: EDGE.right, top: EDGE.top, height: price },
      { left: EDGE.left, right: EDGE.right, top: EDGE.top + Math.round(price * 0.8), height: Math.round(price * 0.2) },
    ];
    for (var i = 0; i < panes; i++) {
      grids.push({ left: EDGE.left, right: EDGE.right, top: EDGE.top + price + EDGE.gap + i * (pane + EDGE.gap), height: pane });
    }
    return grids;
  }

  // ── Zoom ─────────────────────────────────────────────────────────────────

  function zoomOf(chart, d) {
    var dz = ((chart.getOption() || {}).dataZoom || [])[0] || {};
    var last = d.times.length - 1;
    var from = dz.startValue, to = dz.endValue;
    if (typeof from !== "number" || typeof to !== "number") return { from: 0, to: last };
    return { from: Math.max(0, Math.round(from)), to: Math.min(last, Math.round(to)) };
  }

  function setZoom(chart, d, from, to) {
    var last = d.times.length - 1;
    var span = Math.min(last, Math.max(4, to - from));
    to = Math.min(last, Math.max(span, to));
    from = Math.max(0, to - span);
    chart.dispatchAction({ type: "dataZoom", dataZoomIndex: 0, startValue: from, endValue: to });
  }

  // Labels and gridlines for the stretch now on screen. The step only
  // changes with the zoom, so panning costs nothing.
  function updateTicks(chart, d, force) {
    var z = zoomOf(chart, d);
    var width = chart.getWidth() - EDGE.left - EDGE.right;
    var step = stepFor(d, z.to - z.from + 1, width);
    if (!force && d.tickStep === step.id && d.tickCount === d.times.length) return;
    d.tickStep = step.id;
    d.tickCount = d.times.length;
    var set = boundsOf(d, step).set;
    var shows = function (index) { return !!set[index]; };
    var last = d.grids.length - 1;
    chart.setOption({
      xAxis: d.grids.map(function (_, i) {
        return {
          axisLabel: { interval: shows, formatter: function (value) { return tickLabel(d, +value); } },
          splitLine: { show: i !== 1, interval: shows },
          axisTick: { show: i === last, interval: shows },
        };
      }),
    });
  }

  // ── The readout ──────────────────────────────────────────────────────────

  function el(parent, tag, style, text) {
    var node = document.createElement(tag);
    if (style) node.style.cssText = style;
    if (text !== undefined) node.textContent = text;
    parent.appendChild(node);
    return node;
  }

  // The figures at the crosshair, or the newest candle's: open, high, low,
  // close, change and volume, each overlay's value, and each pane's lines.
  // Built once per draw; `renderLegend` only swaps the text.
  function buildLegend(d) {
    var host = document.getElementById(d.id + "-legend");
    d.legend = null;
    if (!host) return;
    host.textContent = "";
    var col = d.col, L = d.cfg.labels || {};
    var halo = "text-shadow:0 0 3px " + col.base + ",0 0 3px " + col.base + ",0 0 6px " + col.base + ";";
    var row = "display:flex;flex-wrap:wrap;align-items:baseline;column-gap:10px;row-gap:0;font-size:11.5px;line-height:1.55;font-variant-numeric:tabular-nums;" + halo;
    var main = el(host, "div", "position:absolute;left:" + (EDGE.left + 6) + "px;top:" + (EDGE.top + 2) + "px;right:" + (EDGE.right + 6) + "px;" + row);
    var legend = { ohlc: {}, lines: [], panes: [] };
    function pair(parent, label, color) {
      var box = el(parent, "span", "white-space:nowrap;");
      el(box, "span", "color:" + (color || col.sub) + ";margin-right:4px;", label);
      return el(box, "span", "color:" + col.text + ";font-weight:600;");
    }
    ["o", "h", "l", "c"].forEach(function (k) { legend.ohlc[k] = pair(main, L[k] || k.toUpperCase()); });
    legend.change = el(main, "span", "white-space:nowrap;font-weight:600;");
    if (d.cfg.volumeShown) legend.volume = pair(main, L.vol || "Vol");
    if (d.cfg.note) el(main, "span", "color:" + col.faint + ";white-space:nowrap;", d.cfg.note);
    // A study drawn as two lines that take turns (a stop under the price,
    // then over it) reads as one: `alt` is the other line.
    var studies = (d.cfg.overlays || []).filter(function (o) { return !o.quiet; }).map(function (o) { return { id: o.id, name: o.name, color: d.c(o.color), alt: o.alt }; });
    (d.cfg.bands || []).forEach(function (b) {
      studies.push({ id: b.id + "u", name: b.name, color: d.c(b.color), also: b.id + "l" });
    });
    if (studies.length) {
      el(main, "span", "flex-basis:100%;height:0;");
      studies.forEach(function (s) {
        var value = pair(main, s.name, s.color);
        value.style.color = s.color;
        legend.lines.push({ id: s.id, also: s.also, alt: s.alt, node: value });
      });
    }
    (d.cfg.panes || []).forEach(function (p, i) {
      var grid = d.grids[2 + i];
      var box = el(host, "div", "position:absolute;left:" + (EDGE.left + 6) + "px;top:" + (grid.top + 1) + "px;right:" + (EDGE.right + 6) + "px;" + row);
      el(box, "span", "color:" + col.sub + ";font-weight:600;white-space:nowrap;", p.title);
      (p.lines || []).concat(p.bars || []).forEach(function (line) {
        var color = line.color ? d.c(line.color) : col.text;
        var value = pair(box, (p.lines || []).length + (p.bars || []).length > 1 ? line.name : "", color);
        value.style.color = color;
        legend.panes.push({ id: line.id, node: value, signed: !line.color });
      });
    });
    d.legend = legend;
    renderLegend(d);
  }

  function renderLegend(d) {
    var legend = d.legend;
    if (!legend) return;
    var n = d.times.length - 1;
    var i = d.hover === null || d.hover === undefined ? n : Math.max(0, Math.min(n, d.hover));
    var o = d.ohlc[i]; // [open, close, low, high]
    if (!o) return;
    var col = d.col;
    var tone = o[1] >= o[0] ? col.up : col.down;
    [["o", 0], ["h", 3], ["l", 2], ["c", 1]].forEach(function (k) {
      legend.ohlc[k[0]].textContent = fmtPrice(o[k[1]]);
      legend.ohlc[k[0]].style.color = tone;
    });
    // Against the candle before; the first candle against its own open.
    var base = i > 0 ? d.ohlc[i - 1][1] : o[0];
    var diff = o[1] - base;
    var pct = base ? diff / base * 100 : 0;
    // To the price's own decimals: a few cents on a $165 stock is 0.33, not 0.3250.
    var digits = Math.abs(o[1]) >= 1 ? 2 : 4;
    var moved = Math.abs(diff).toLocaleString(undefined, { minimumFractionDigits: digits, maximumFractionDigits: digits });
    legend.change.textContent = (diff >= 0 ? "+" : "−") + moved + " (" + (diff >= 0 ? "+" : "−") + Math.abs(pct).toFixed(2) + "%)";
    legend.change.style.color = diff >= 0 ? col.up : col.down;
    if (legend.volume) legend.volume.textContent = compact(d.volume[i] || 0);
    function at(id) {
      var line = d.lines[id];
      var v = line ? line[i] : null;
      return v === null || v === undefined ? null : v;
    }
    legend.lines.forEach(function (s) {
      var v = at(s.id);
      if (v === null && s.alt) v = at(s.alt);
      var also = s.also ? at(s.also) : null;
      s.node.textContent = v === null ? "—" : fmtPrice(v) + (also === null ? "" : " / " + fmtPrice(also));
    });
    legend.panes.forEach(function (s) {
      var v = at(s.id);
      s.node.textContent = v === null ? "—" : fmtValue(v);
      if (s.signed && v !== null) s.node.style.color = v >= 0 ? col.up : col.down;
    });
  }

  // ── Drawings ─────────────────────────────────────────────────────────────

  // markLine items for the saved drawings, the one being placed, and a measure.
  function drawingLines(id, d) {
    var out = [];
    var col = d.col;
    var last = d.times.length - 1;
    if (last < 0) return out;
    // A point before the first candle sits on it; a drawing entirely before
    // this span's candles (or after them) isn't shown.
    function at(p) { var i = indexAt(d.times, p.ts); return i < 0 ? 0 : i; }
    function visible(points) {
      return points.some(function (p) { return p.ts >= d.times[0]; }) || points.length === 1;
    }
    function line(name, a, b, color, label, dashed) {
      return [
        { name: name, coord: [a[0], d.sy(a[1])], lineStyle: { color: color, width: 1.5, type: dashed ? "dashed" : "solid" },
          label: { show: !!label, formatter: label || "", position: "insideEndTop", color: color, fontSize: 11 } },
        { coord: [b[0], d.sy(b[1])] },
      ];
    }
    function add(name, kind, points, dashed) {
      if (!points.length || !visible(points)) return;
      if (kind === "Level") {
        out.push({ name: name, yAxis: d.sy(points[0].price),
          lineStyle: { color: col.peach, width: 1.5, type: dashed ? "dashed" : "solid" },
          label: { show: true, formatter: fmtPrice(points[0].price), position: "insideEndTop", color: col.peach, fontSize: 11 } });
      } else if (kind === "Trend" && points.length === 2) {
        out.push(line(name, [at(points[0]), points[0].price], [at(points[1]), points[1].price], col.sky, "", dashed));
      } else if (kind === "Fibonacci" && points.length === 2) {
        var from = Math.min(at(points[0]), at(points[1]));
        FIB_LEVELS.forEach(function (l) {
          var price = points[1].price - (points[1].price - points[0].price) * l;
          out.push(line(name, [from, price], [last, price], col.yellow,
            (l * 100).toFixed(1).replace(/\.0$/, "") + "% · " + fmtPrice(price), dashed));
        });
      }
    }
    (SC.drawings[id] || []).forEach(function (dr) { add("d" + dr.id, dr.kind, dr.points, false); });
    var t = SC.tools[id];
    if (t && t.first && t.preview) {
      var kind = { trend: "Trend", fibonacci: "Fibonacci", measure: "Trend" }[t.tool];
      if (kind) add("preview", kind, [t.first, t.preview], true);
    }
    if (t && t.measure) {
      var a = t.measure[0], b = t.measure[1];
      var pct = (b.price / a.price - 1) * 100;
      var bars = Math.abs(at(b) - at(a));
      out.push(line("measure", [at(a), a.price], [at(b), b.price], pct >= 0 ? col.up : col.down,
        (pct >= 0 ? "+" : "") + pct.toFixed(2) + "% · " + bars + " " + (d.cfg.measureUnit || ""), true));
    }
    return out;
  }

  function renderDrawings(id) {
    var chart = live(id);
    if (!chart) return;
    chart.setOption({ series: [{ id: "drawings", markLine: { data: drawingLines(id, chart.__data) } }] });
  }

  // Clicks place points for the tool in use; a click on a drawing with the
  // eraser removes it. Double-click goes back to the span first shown, and
  // the keyboard pans and zooms.
  function attach(chart, id, host) {
    if (chart.__attached) return;
    chart.__attached = true;
    function emit(msg) { var cb = SC.listeners[id]; if (cb) cb(msg); }
    function pointAt(x, y) {
      var d = chart.__data;
      if (!d || !chart.containPixel({ gridIndex: 0 }, [x, y])) return null;
      var v = chart.convertFromPixel({ gridIndex: 0 }, [x, y]);
      var i = Math.max(0, Math.min(d.times.length - 1, Math.round(v[0])));
      var price = d.iy(v[1]);
      if (!(price > 0) || d.times[i] === undefined) return null;
      return { ts: d.times[i], price: Math.round(price * 1e4) / 1e4 };
    }
    var zr = chart.getZr();
    zr.on("click", function (e) {
      var t = SC.tools[id];
      if (!t || !t.tool || t.tool === "erase") return;
      var p = pointAt(e.offsetX, e.offsetY);
      if (!p) return;
      if (t.tool === "level") { emit({ add: "Level", points: [p] }); return; }
      if (!t.first) { t.first = p; t.measure = null; renderDrawings(id); return; }
      var first = t.first;
      t.first = null; t.preview = null;
      if (t.tool === "measure") t.measure = [first, p];
      else emit({ add: t.tool === "trend" ? "Trend" : "Fibonacci", points: [first, p] });
      renderDrawings(id);
    });
    zr.on("dblclick", function () {
      var t = SC.tools[id];
      if (!t || !t.tool) SC.command(id, "reset");
    });
    var queued = false;
    zr.on("mousemove", function (e) {
      var t = SC.tools[id];
      if (!t || !t.first) return;
      t.preview = pointAt(e.offsetX, e.offsetY) || t.preview;
      if (queued) return;
      queued = true;
      requestAnimationFrame(function () { queued = false; renderDrawings(id); });
    });
    zr.on("globalout", function () {
      var d = chart.__data;
      if (d && d.hover !== null) { d.hover = null; renderLegend(d); }
    });
    chart.on("click", function (params) {
      var t = SC.tools[id];
      if (!t || t.tool !== "erase" || params.componentType !== "markLine") return;
      var name = (params.data && params.data.name) || params.name || "";
      if (/^d\d+$/.test(name)) emit({ erase: Number(name.slice(1)) });
    });
    chart.on("updateAxisPointer", function (e) {
      var d = chart.__data;
      if (!d) return;
      var info = (e.axesInfo || []).filter(function (a) { return a.axisDim === "x"; })[0];
      var next = info ? Math.round(+info.value) : null;
      if (next === d.hover) return;
      d.hover = next;
      renderLegend(d);
    });
    var pending = false;
    chart.on("dataZoom", function () {
      if (pending) return;
      pending = true;
      requestAnimationFrame(function () {
        pending = false;
        if (live(id)) updateTicks(chart, chart.__data, false);
      });
    });
    // Keys, while the chart has the focus (click it, or Tab to it).
    host.tabIndex = 0;
    host.style.outline = "none";
    host.addEventListener("keydown", function (e) {
      var keys = { ArrowLeft: "left", ArrowRight: "right", ArrowUp: "in", ArrowDown: "out", "+": "in", "=": "in", "-": "out", End: "latest", Home: "reset" };
      if (e.key === "Escape") {
        var t = SC.tools[id];
        if (t && t.tool) { emit({ cancel: true }); e.stopPropagation(); }
        return;
      }
      if (!keys[e.key] || e.ctrlKey || e.metaKey || e.altKey) return;
      e.preventDefault();
      SC.command(id, keys[e.key]);
    });
  }

  // ── Series ───────────────────────────────────────────────────────────────

  function volumeData(d) {
    return d.volume.map(function (v, i) {
      var o = d.ohlc[i];
      return { value: v, itemStyle: { color: (o[1] >= o[0] ? d.col.up : d.col.down) + "66" } };
    });
  }

  function priceData(d) {
    var sy = d.sy;
    if (d.style === "line" || d.style === "area") return d.ohlc.map(function (o) { return sy(o[1]); });
    if (d.style === "bars") return d.shown.map(function (o, i) { return [i, sy(o[0]), sy(o[1]), sy(o[2]), sy(o[3])]; });
    return d.shown.map(function (o) { return [sy(o[0]), sy(o[1]), sy(o[2]), sy(o[3])]; });
  }

  function lineData(d, id) {
    return d.lines[id].map(function (v) { return v === null || v === undefined ? null : d.sy(v); });
  }
  function plainData(d, id) {
    return d.lines[id].map(function (v) { return v === null || v === undefined ? null : v; });
  }
  // The band between a lower and an upper line is drawn by stacking the
  // difference on the lower one.
  function bandGap(d, band) {
    var up = d.lines[band.id + "u"], lo = d.lines[band.id + "l"];
    return up.map(function (v, i) {
      var l = lo[i];
      return v === null || v === undefined || l === null || l === undefined ? null : d.sy(v) - d.sy(l);
    });
  }
  function histogram(d, id) {
    return d.lines[id].map(function (v) {
      return v === null || v === undefined ? null : { value: v, itemStyle: { color: (v >= 0 ? d.col.up : d.col.down) + "aa" } };
    });
  }

  // The price line, where the stock last closed, and the viewer's cost.
  function markLines(d) {
    var col = d.col, cfg = d.cfg, L = cfg.labels || {};
    var out = [];
    function level(price, color, text, solidTag) {
      out.push({
        yAxis: d.sy(price), lineStyle: { color: color, width: 1, type: "dashed", opacity: solidTag ? 0.9 : 0.6 },
        label: solidTag
          ? { show: true, position: "end", formatter: text, color: col.crust, backgroundColor: color, padding: [3, 5], borderRadius: 3, fontSize: 11, fontWeight: 600, distance: 2 }
          : { show: true, position: "insideEndTop", formatter: text, color: color, fontSize: 10.5 },
      });
    }
    if (cfg.prevClose > 0) level(cfg.prevClose, col.faint, (L.prev || "Prev close") + " " + fmtPrice(cfg.prevClose), false);
    if (cfg.cost > 0) level(cfg.cost, col.mauve, (L.cost || "Avg cost") + " " + fmtPrice(cfg.cost), false);
    var n = d.ohlc.length - 1;
    if (n >= 0) {
      var last = d.ohlc[n];
      var base = cfg.prevClose > 0 ? cfg.prevClose : (n > 0 ? d.ohlc[n - 1][1] : last[0]);
      level(last[1], last[1] >= base ? col.up : col.down, fmtPrice(last[1]), true);
    }
    return out;
  }

  // Everything a streamed trade can move.
  function liveSeries(d) {
    var cfg = d.cfg;
    var out = [{ id: "price", data: priceData(d) }, { id: "marks", markLine: { data: markLines(d) } }];
    if (cfg.volumeShown) out.push({ id: "volume", data: volumeData(d) });
    (cfg.overlays || []).forEach(function (o) { out.push({ id: o.id, data: lineData(d, o.id) }); });
    (cfg.bands || []).forEach(function (b) {
      out.push({ id: b.id + "l", data: lineData(d, b.id + "l") });
      out.push(d.log ? { id: b.id + "u", data: lineData(d, b.id + "u") } : { id: b.id + "u", data: bandGap(d, b) });
    });
    (cfg.panes || []).forEach(function (p) {
      (p.bars || []).forEach(function (b) { out.push({ id: b.id, data: histogram(d, b.id) }); });
      (p.lines || []).forEach(function (l) { out.push({ id: l.id, data: plainData(d, l.id) }); });
    });
    return out;
  }

  function draw(host, id, cfg) {
    var css = getComputedStyle(host);
    function c(name) { return css.getPropertyValue("--catppuccin-color-" + name).trim(); }
    var col = {
      text: c("text"), sub: c("subtext0"), faint: c("overlay0"), grid: c("surface0"),
      up: c("green"), down: c("red"), mauve: c("mauve"), base: c("mantle"), crust: c("crust"),
      peach: c("peach"), sky: c("sky"), yellow: c("yellow"),
    };
    var chart = SC.charts[id];
    var fresh = !chart || chart.isDisposed() || chart.getDom() !== host;
    if (fresh) {
      if (chart && !chart.isDisposed()) chart.dispose();
      chart = echarts.init(host, null, { renderer: "canvas" });
      SC.charts[id] = chart;
      // The plots are laid out in pixels, so a new size lays them out again.
      var w = host.clientWidth, h = host.clientHeight, queued = false;
      var observer = window.ResizeObserver ? new ResizeObserver(function () {
        if (queued) return;
        queued = true;
        requestAnimationFrame(function () {
          queued = false;
          if (chart.isDisposed() || (host.clientWidth === w && host.clientHeight === h)) return;
          w = host.clientWidth; h = host.clientHeight;
          chart.resize();
          var d = chart.__data;
          if (!d) return;
          d.grids = layoutOf(host, (d.cfg.panes || []).length);
          chart.setOption({ grid: d.grids });
          buildLegend(d);
          updateTicks(chart, d, true);
        });
      }) : null;
      if (observer) observer.observe(host);
      chart.__unwatch = function () { if (observer) observer.disconnect(); };
    }

    var log = !!cfg.log;
    var n = cfg.times.length;
    var panes = cfg.panes || [];
    var d = {
      id: id, cfg: cfg, col: col, c: c, log: log,
      // On a log scale the plot holds log10 of each price.
      sy: log ? function (v) { return v > 0 ? Math.log(v) / Math.LN10 : null; } : function (v) { return v; },
      iy: log ? function (v) { return Math.pow(10, v); } : function (v) { return v; },
      times: cfg.times.slice(),
      cats: cfg.times.map(function (_, i) { return String(i); }),
      ohlc: cfg.ohlc.map(function (o) { return o.slice(); }),
      shown: (cfg.shown || cfg.ohlc).map(function (o) { return o.slice(); }),
      volume: cfg.volume.slice(),
      lines: {}, step: cfg.step || DAY, intraday: !!cfg.intraday, style: cfg.style,
      hover: null, local: null, bounds: {}, tickStep: null,
      grids: layoutOf(host, panes.length),
    };
    (cfg.overlays || []).forEach(function (o) { d.lines[o.id] = o.values.slice(); });
    (cfg.bands || []).forEach(function (b) { d.lines[b.id + "u"] = b.upper.slice(); d.lines[b.id + "l"] = b.lower.slice(); });
    panes.forEach(function (p) {
      (p.lines || []).concat(p.bars || []).forEach(function (l) { d.lines[l.id] = l.values.slice(); });
    });

    // A redraw of the same view (new candles, an indicator toggled) keeps
    // the viewer's zoom, following the newest candle if they were on it; a
    // new span starts from its own.
    var zoom = { from: Math.max(0, Math.min(n - 1, cfg.zoom.from)), to: n - 1 };
    var before = chart.__data;
    if (before && chart.__view === cfg.view) {
      var z = zoomOf(chart, before);
      var wasLast = before.times.length - 1;
      if (z.to >= wasLast) zoom = { from: Math.max(0, n - 1 - (z.to - z.from)), to: n - 1 };
      else {
        // The same candles by time, wherever they now sit.
        var a = indexAt(d.times, before.times[z.from]), b = indexAt(d.times, before.times[z.to]);
        if (a >= 0 && b > a) zoom = { from: a, to: b };
      }
    }
    chart.__view = cfg.view;

    var grids = d.grids;
    var lastGrid = grids.length - 1;
    var pointer = { backgroundColor: col.grid, color: col.text, fontSize: 11 };
    var xAxes = grids.map(function (_, i) {
      return {
        type: "category", gridIndex: i, data: d.cats, boundaryGap: true,
        axisLine: { show: i === lastGrid, lineStyle: { color: col.grid } },
        axisTick: { show: false, lineStyle: { color: col.grid } },
        // The first and last labels are kept whole inside the plot.
        axisLabel: { show: i === lastGrid, color: col.faint, fontSize: 11, hideOverlap: true, alignMinLabel: "left", alignMaxLabel: "right" },
        splitLine: { show: false, lineStyle: { color: col.grid, opacity: 0.7 } },
        axisPointer: { label: Object.assign({ show: i === lastGrid, formatter: function (p) { return fullLabel(d, +p.value); } }, pointer) },
      };
    });
    function yAxis(i, opts) {
      return Object.assign({
        gridIndex: i, position: "right", scale: true, splitNumber: 2,
        axisLine: { show: false }, axisTick: { show: false },
        axisLabel: { color: col.faint, fontSize: 11, formatter: shortNumber },
        splitLine: { lineStyle: { color: col.grid, opacity: 0.7 } },
        axisPointer: { label: Object.assign({ formatter: function (p) { return fmtValue(p.value); } }, pointer) },
      }, opts || {});
    }
    var yAxes = [
      yAxis(0, {
        splitNumber: 6,
        axisLabel: { color: col.sub, fontSize: 11, formatter: function (v) { return fmtPrice(d.iy(v)); } },
        axisPointer: { label: Object.assign({ formatter: function (p) { return fmtPrice(d.iy(p.value)); } }, pointer) },
      }),
      yAxis(1, { show: false, scale: false, axisPointer: { show: false } }),
    ];
    panes.forEach(function (p, i) {
      yAxes.push(p.bounded
        ? yAxis(2 + i, { min: p.bounded[0], max: p.bounded[1], interval: (p.bounded[1] - p.bounded[0]) / 2, scale: false })
        : yAxis(2 + i));
    });

    var series = [];
    var candle = { color: col.up, color0: col.down, borderColor: col.up, borderColor0: col.down };
    var price;
    if (cfg.style === "line" || cfg.style === "area") {
      price = {
        id: "price", name: "Price", type: "line", data: priceData(d), showSymbol: false,
        lineStyle: { color: col.mauve, width: 2 }, itemStyle: { color: col.mauve }, z: 4,
        areaStyle: cfg.style === "area" ? { origin: "start", color: new echarts.graphic.LinearGradient(0, 0, 0, 1, [
          { offset: 0, color: col.mauve + "55" }, { offset: 1, color: col.mauve + "00" },
        ]) } : undefined,
      };
    } else if (cfg.style === "bars") {
      // Open tick to the left, close tick to the right of a high–low line.
      price = {
        id: "price", name: "Price", type: "custom", data: priceData(d), encode: { x: 0, y: [1, 2, 3, 4] }, z: 4,
        renderItem: function (params, api) {
          var i = api.value(0);
          var open = api.coord([i, api.value(1)]), close = api.coord([i, api.value(2)]);
          var low = api.coord([i, api.value(3)]), high = api.coord([i, api.value(4)]);
          var half = Math.max(1.5, Math.min(8, api.size([1, 0])[0] * 0.32));
          var style = { stroke: api.value(2) >= api.value(1) ? col.up : col.down, lineWidth: half > 3 ? 1.5 : 1 };
          return { type: "group", children: [
            { type: "line", shape: { x1: high[0], y1: high[1], x2: low[0], y2: low[1] }, style: style },
            { type: "line", shape: { x1: open[0] - half, y1: open[1], x2: open[0], y2: open[1] }, style: style },
            { type: "line", shape: { x1: close[0], y1: close[1], x2: close[0] + half, y2: close[1] }, style: style },
          ] };
        },
      };
    } else {
      price = {
        id: "price", name: "Price", type: "candlestick", data: priceData(d), barMaxWidth: 20, z: 4,
        itemStyle: cfg.style === "hollow" ? Object.assign({}, candle, { color: "transparent" }) : candle,
      };
    }
    series.push(price);

    (cfg.bands || []).forEach(function (b) {
      var color = c(b.color);
      var plain = { type: "line", showSymbol: false, silent: true, emphasis: { disabled: true }, lineStyle: { color: color, width: 1, opacity: 0.8 }, z: 2 };
      series.push(Object.assign({ id: b.id + "l", name: b.name + " lower", data: lineData(d, b.id + "l") }, plain, log ? {} : { stack: b.id }));
      series.push(Object.assign({ id: b.id + "u", name: b.name + " upper" }, plain, log
        ? { data: lineData(d, b.id + "u") }
        : { data: bandGap(d, b), stack: b.id, areaStyle: { color: color, opacity: 0.08 } }));
    });
    (cfg.overlays || []).forEach(function (o) {
      var color = c(o.color);
      // Dots are a line with only its points drawn, so the slim ECharts
      // build (js/echarts.entry.js) needn't carry the scatter chart.
      series.push(o.dots ? {
        id: o.id, name: o.name, type: "line", data: lineData(d, o.id), silent: true,
        showSymbol: true, showAllSymbol: true, symbol: "circle", symbolSize: 3.5,
        lineStyle: { opacity: 0 }, itemStyle: { color: color }, emphasis: { disabled: true }, z: 3,
      } : {
        id: o.id, name: o.name, type: "line", data: lineData(d, o.id), showSymbol: false, connectNulls: false, silent: true,
        lineStyle: { color: color, width: o.dashed ? 1 : 1.5, type: o.dashed ? "dashed" : "solid" },
        emphasis: { disabled: true }, z: 3,
      });
    });
    if (cfg.volumeShown) {
      series.push({ id: "volume", name: "Volume", type: "bar", xAxisIndex: 1, yAxisIndex: 1, data: volumeData(d), barMaxWidth: 20, silent: true, z: 1 });
    }
    panes.forEach(function (p, i) {
      (p.bars || []).forEach(function (b) {
        series.push({ id: b.id, name: b.name, type: "bar", xAxisIndex: 2 + i, yAxisIndex: 2 + i, data: histogram(d, b.id), barMaxWidth: 20, silent: true });
      });
      (p.lines || []).forEach(function (l, k) {
        series.push({
          id: l.id, name: l.name, type: "line", xAxisIndex: 2 + i, yAxisIndex: 2 + i, data: plainData(d, l.id), showSymbol: false, silent: true,
          lineStyle: { color: c(l.color), width: 1.5 }, emphasis: { disabled: true },
          markLine: k === 0 && (p.levels || []).length ? {
            silent: true, symbol: "none", label: { show: false },
            lineStyle: { color: col.faint, type: "dashed", opacity: 0.8 },
            data: p.levels.map(function (y) { return { yAxis: y }; }),
          } : undefined,
        });
      });
    });

    // Trading outside regular hours sits on a shaded ground.
    var shaded = [];
    if ((cfg.regular || []).length) {
      var start = null;
      var inside = function (t) { return cfg.regular.some(function (r) { return t >= r[0] && t <= r[1]; }); };
      for (var i = 0; i <= n; i++) {
        var off = i < n && !inside(d.times[i]);
        if (off && start === null) start = i;
        if (!off && start !== null) { shaded.push([{ xAxis: start }, { xAxis: i - 1 }]); start = null; }
      }
    }
    // The viewer's own trades: an arrow under the candle bought on, over
    // the one sold on; pointing at it says what was traded.
    var trades = (cfg.trades || []).filter(function (t) { return d.ohlc[t.i]; }).map(function (t) {
      var o = d.ohlc[t.i];
      return {
        coord: [t.i, d.sy(t.buy ? o[2] : o[3])], text: t.text,
        symbolRotate: t.buy ? 0 : 180, symbolOffset: [0, t.buy ? 12 : -12],
        itemStyle: { color: t.buy ? col.up : col.down, borderColor: col.base, borderWidth: 1 },
      };
    });
    // Levels, shading and trades live on an empty series, so a streamed
    // trade can move the price line without the candles being sent again.
    series.push({
      id: "marks", name: "Marks", type: "line", data: [], z: 6,
      markLine: { symbol: ["none", "none"], animation: false, silent: true, data: markLines(d) },
      markArea: { silent: true, itemStyle: { color: col.faint, opacity: 0.16 }, data: shaded },
      markPoint: {
        symbol: "triangle", symbolSize: 11, animation: false, data: trades,
        label: { show: false },
        emphasis: { scale: 1.3, label: {
          show: true, position: "top", distance: 10, formatter: function (p) { return p.data.text; },
          color: col.text, backgroundColor: col.base, borderColor: col.grid, borderWidth: 1, padding: [5, 7], borderRadius: 6, fontSize: 11.5, lineHeight: 16,
        } },
      },
    });
    // Drawings live on an empty series so they move with zoom and pan.
    series.push({
      id: "drawings", name: "Drawings", type: "line", data: [], silent: false,
      markLine: { symbol: ["none", "none"], animation: false, silent: false, data: drawingLines(id, d),
        emphasis: { lineStyle: { width: 3 } } },
    });

    var allAxes = grids.map(function (_, i) { return i; });
    chart.__data = d;
    chart.setOption({
      animation: false,
      backgroundColor: "transparent",
      textStyle: { color: col.text },
      grid: grids,
      xAxis: xAxes,
      yAxis: yAxes,
      axisPointer: { link: [{ xAxisIndex: "all" }], label: pointer, lineStyle: { color: col.faint, type: "dashed" } },
      // The readout above the plots says what a tooltip would; the
      // crosshair alone follows the pointer, so nothing covers the candles.
      tooltip: { trigger: "axis", showContent: false, axisPointer: { type: "cross", crossStyle: { color: col.faint } } },
      dataZoom: [
        // The wheel scrolls the page; Ctrl+wheel (or pinch) zooms, drag pans.
        { id: "inside", type: "inside", xAxisIndex: allAxes, startValue: zoom.from, endValue: zoom.to, minValueSpan: 4,
          zoomOnMouseWheel: SC.full[id] ? true : "ctrl", moveOnMouseWheel: false, moveOnMouseMove: true },
        { id: "slider", type: "slider", xAxisIndex: allAxes, startValue: zoom.from, endValue: zoom.to, minValueSpan: 4,
          bottom: 2, height: 16, showDetail: false, brushSelect: false,
          borderColor: "transparent", backgroundColor: "transparent", fillerColor: col.mauve + "22",
          handleStyle: { color: col.mauve, borderColor: col.mauve }, moveHandleSize: 0,
          dataBackground: { lineStyle: { color: col.faint, opacity: 0.6 }, areaStyle: { color: col.grid, opacity: 0.5 } },
          selectedDataBackground: { lineStyle: { color: col.mauve, opacity: 0.8 }, areaStyle: { color: col.mauve, opacity: 0.15 } } },
      ],
      series: series,
    }, true);
    attach(chart, id, host);
    var tool = SC.tools[id];
    if (tool && tool.tool) chart.getZr().setCursorStyle("crosshair");
    buildLegend(d);
    updateTicks(chart, d, true);
  }
})();
