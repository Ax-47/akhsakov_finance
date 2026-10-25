// Technical stock chart: candles or a line with overlays (moving averages,
// Bollinger bands), volume, and an RSI or MACD pane. Rust computes every
// series; this file only draws, and moves the last candle with each live
// price (StockChart.tick). It also draws the lines the viewer places
// (levels, trend lines, Fibonacci retracements, a measure); Rust saves them.
// Loads ECharts through GrowthChart.ready.
window.StockChart = window.StockChart || {};
window.StockChart.latest = window.StockChart.latest || {};
window.StockChart.charts = window.StockChart.charts || {};
// Per chart: saved drawings, the tool in use, and where placed points go.
window.StockChart.drawings = window.StockChart.drawings || {};
window.StockChart.tools = window.StockChart.tools || {};
window.StockChart.listeners = window.StockChart.listeners || {};

var FIB_LEVELS = [0, 0.236, 0.382, 0.5, 0.618, 0.786, 1];

// Saved drawings: [{ id, kind: "Level" | "Trend" | "Fibonacci", points: [{ ts, price }] }].
window.StockChart.setDrawings = function (id, list) {
  window.StockChart.drawings[id] = list || [];
  renderDrawings(id);
};

// The tool clicks place: "level", "trend", "fibonacci", "measure", "erase" or null.
window.StockChart.setTool = function (id, tool) {
  window.StockChart.tools[id] = { tool: tool || null, first: null, preview: null, measure: null };
  var chart = window.StockChart.charts[id];
  if (chart && !chart.isDisposed()) chart.getZr().setCursorStyle(tool ? "crosshair" : "default");
  renderDrawings(id);
};

// Placed drawings and erase clicks go to `cb`: { add, points } or { erase }.
window.StockChart.listen = function (id, cb) {
  window.StockChart.listeners[id] = cb;
};

// Frees a chart whose component unmounted (the chart map kept every
// chart, and its detached DOM, alive before).
window.StockChart.dispose = function (id) {
  delete window.StockChart.latest[id];
  delete window.StockChart.drawings[id];
  delete window.StockChart.tools[id];
  delete window.StockChart.listeners[id];
  var chart = window.StockChart.charts[id];
  delete window.StockChart.charts[id];
  if (!chart) return;
  if (chart.__unwatch) chart.__unwatch();
  if (!chart.isDisposed()) chart.dispose();
};

window.StockChart.init = function (id, cfg) {
  window.StockChart.latest[id] = cfg;
  var ready = window.GrowthChart && window.GrowthChart.ready
    ? window.GrowthChart.ready()
    : Promise.reject(new Error("growth_chart.js not loaded"));
  ready.then(function () {
    if (window.StockChart.latest[id] !== cfg) return; // superseded
    var el = document.getElementById(id);
    if (!el) return;
    draw(el, id, cfg);
  }).catch(function (e) { console.error("[StockChart]", e); });
};

// Moves the chart with a streamed price: the last candle's close (and its
// high or low), or a new candle once the last one's time is up. The
// indicators catch up on the next fetch.
window.StockChart.tick = function (id, price, ts) {
  var chart = window.StockChart.charts[id];
  var d = chart && !chart.isDisposed() && chart.__data;
  if (!d || !d.times.length || !(price > 0)) return;
  var n = d.times.length - 1;
  var start = d.times[n];
  if (ts < start) return;
  // A daily candle belongs to its UTC day, like the fetched ones.
  var next = d.step >= 86400 && d.step < 7 * 86400
    ? Math.floor(ts / 86400) > Math.floor(start / 86400)
    : ts >= start + d.step;
  if (next) {
    var t = d.step >= 86400 && d.step < 7 * 86400
      ? start + (Math.floor(ts / 86400) - Math.floor(start / 86400)) * 86400
      : start + Math.floor((ts - start) / d.step) * d.step;
    d.times.push(t);
    d.dates.push(d.intraday ? fullTime(t) : new Date(t * 1000).toISOString().slice(0, 10));
    d.ohlc.push([price, price, price, price]);
    d.close.push(price);
    d.volume.push(0);
    d.up.push(true);
  } else {
    var o = d.ohlc[n]; // [open, close, low, high]
    if (o[1] === price) return;
    o[1] = price;
    o[2] = Math.min(o[2], price);
    o[3] = Math.max(o[3], price);
    d.close[n] = price;
    d.up[n] = price >= o[0];
  }
  var axes = [];
  for (var i = 0; i < d.axes; i++) axes.push({ data: d.dates.slice() });
  var series = [
    { id: "price", data: d.style === "candle" ? d.ohlc.map(function (o) { return o.slice(); }) : d.close.slice() },
    { id: "volume", data: volumeData(d, d.col) },
  ];
  // Fibonacci levels run to the last candle.
  if (next) series.push({ id: "drawings", markLine: { data: drawingLines(id, d, d.col) } });
  chart.setOption({ xAxis: axes, series: series });
};

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

function fmtPrice(v) { return Number(v).toLocaleString(undefined, { maximumFractionDigits: 2 }); }

// markLine items for the saved drawings, the one being placed, and a measure.
function drawingLines(id, d, col) {
  var out = [];
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
      { name: name, coord: [a[0], a[1]], lineStyle: { color: color, width: 1.5, type: dashed ? "dashed" : "solid" },
        label: { show: !!label, formatter: label || "", position: "insideEndTop", color: color, fontSize: 11 } },
      { coord: [b[0], b[1]] },
    ];
  }
  function add(name, kind, points, dashed) {
    if (!points.length || !visible(points)) return;
    if (kind === "Level") {
      out.push({ name: name, yAxis: points[0].price,
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
  (window.StockChart.drawings[id] || []).forEach(function (dr) { add("d" + dr.id, dr.kind, dr.points, false); });
  var t = window.StockChart.tools[id];
  if (t && t.first && t.preview) {
    var kind = { trend: "Trend", fibonacci: "Fibonacci", measure: "Trend" }[t.tool];
    if (kind) add("preview", kind, [t.first, t.preview], true);
  }
  if (t && t.measure) {
    var a = t.measure[0], b = t.measure[1];
    var pct = (b.price / a.price - 1) * 100;
    var bars = Math.abs(at(b) - at(a));
    out.push(line("measure", [at(a), a.price], [at(b), b.price], pct >= 0 ? col.up : col.down,
      (pct >= 0 ? "+" : "") + pct.toFixed(2) + "% · " + bars + " " + (d.measureUnit || ""), true));
  }
  return out;
}

function renderDrawings(id) {
  var chart = window.StockChart.charts[id];
  var d = chart && !chart.isDisposed() && chart.__data;
  if (!d) return;
  chart.setOption({ series: [{ id: "drawings", markLine: { data: drawingLines(id, d, d.col) } }] });
}

// Clicks place points for the tool in use; a click on a drawing with the
// eraser removes it.
function attachTools(chart, id) {
  if (chart.__tools) return;
  chart.__tools = true;
  function emit(msg) { var cb = window.StockChart.listeners[id]; if (cb) cb(msg); }
  function pointAt(x, y) {
    var d = chart.__data;
    if (!d || !chart.containPixel({ gridIndex: 0 }, [x, y])) return null;
    var v = chart.convertFromPixel({ gridIndex: 0 }, [x, y]);
    var i = Math.max(0, Math.min(d.times.length - 1, Math.round(v[0])));
    if (!(v[1] > 0) || d.times[i] === undefined) return null;
    return { ts: d.times[i], price: Math.round(v[1] * 1e4) / 1e4 };
  }
  var zr = chart.getZr();
  zr.on("click", function (e) {
    var t = window.StockChart.tools[id];
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
  var queued = false;
  zr.on("mousemove", function (e) {
    var t = window.StockChart.tools[id];
    if (!t || !t.first) return;
    t.preview = pointAt(e.offsetX, e.offsetY) || t.preview;
    if (queued) return;
    queued = true;
    requestAnimationFrame(function () { queued = false; renderDrawings(id); });
  });
  chart.on("click", function (params) {
    var t = window.StockChart.tools[id];
    if (!t || t.tool !== "erase" || params.componentType !== "markLine") return;
    var name = (params.data && params.data.name) || params.name || "";
    if (/^d\d+$/.test(name)) emit({ erase: Number(name.slice(1)) });
  });
}

function fullTime(t) {
  return new Date(t * 1000).toLocaleString([], { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit", hour12: false });
}
function shortTime(t) {
  return new Date(t * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false });
}
function volumeData(d, col) {
  return d.volume.map(function (v, i) {
    return { value: v, itemStyle: { color: (d.up[i] ? col.up : col.down) + "88" } };
  });
}

function draw(el, id, cfg) {
  var css = getComputedStyle(el);
  function c(name) { return css.getPropertyValue("--catppuccin-color-" + name).trim(); }
  var col = {
    text: c("text"), sub: c("subtext0"), faint: c("overlay0"), grid: c("surface0"),
    up: c("green"), down: c("red"), mauve: c("mauve"), base: c("mantle"),
    peach: c("peach"), sky: c("sky"), yellow: c("yellow"),
  };
  var chart = window.StockChart.charts[id];
  if (!chart || chart.isDisposed() || chart.getDom() !== el) {
    if (chart && !chart.isDisposed()) chart.dispose();
    chart = echarts.init(el, null, { renderer: "canvas" });
    window.StockChart.charts[id] = chart;
    chart.__unwatch = window.GrowthChart.watchSize(el, chart);
  }

  // Minute candles are labelled in the viewer's local time.
  var times = cfg.times || [];
  var dates = cfg.intraday ? times.map(fullTime) : cfg.dates;
  // A redraw of the same span (new data, an indicator toggled) keeps the
  // viewer's zoom; a new span starts from its own.
  var zoom = { start: cfg.zoomStart, end: 100 };
  if (chart.__view === cfg.view) {
    var dz = (chart.getOption() || {}).dataZoom;
    if (dz && dz[0] && dz[0].end !== undefined) zoom = { start: dz[0].start, end: dz[0].end };
  }
  chart.__view = cfg.view;

  var hasLower = cfg.lower && cfg.lower.kind !== "none";
  var grids = [
    { left: 56, right: 16, top: 12, height: hasLower ? "52%" : "66%" },
    { left: 56, right: 16, top: hasLower ? "60%" : "72%", height: "10%" },
  ];
  if (hasLower) grids.push({ left: 56, right: 16, top: "74%", height: "14%" });

  function xAxis(i) {
    return {
      type: "category", gridIndex: i, data: dates, boundaryGap: true,
      axisLine: { lineStyle: { color: col.grid } }, axisTick: { show: false },
      axisLabel: {
        show: i === grids.length - 1, color: col.faint, fontSize: 12,
        formatter: cfg.intraday ? function (v, k) { return times[k] ? shortTime(times[k]) : v; } : undefined,
      },
      axisPointer: { label: { show: i === grids.length - 1 } },
    };
  }
  function yAxis(i, opts) {
    return Object.assign({
      gridIndex: i, scale: true, splitNumber: 3,
      axisLabel: { color: col.faint, fontSize: 12 },
      splitLine: { lineStyle: { color: col.grid, type: "dashed" } },
    }, opts || {});
  }
  var xAxes = grids.map(function (_, i) { return xAxis(i); });
  var yAxes = [
    yAxis(0, { axisLabel: { color: col.faint, fontSize: 12, formatter: function (v) { return cfg.symbol + v.toLocaleString(undefined, { maximumFractionDigits: 2 }); } } }),
    yAxis(1, { splitNumber: 1, axisLabel: { show: false }, splitLine: { show: false } }),
  ];
  if (hasLower) {
    // RSI and Stochastic run 0–100; the others take their own scale (OBV
    // can run to billions, so its labels are short).
    yAxes.push(cfg.lower.bounded
      ? yAxis(2, { min: 0, max: 100, interval: 50, scale: false })
      : yAxis(2, { splitNumber: 2, axisLabel: { color: col.faint, fontSize: 12, formatter: shortNumber } }));
  }

  var series = [];
  if (cfg.style === "candle") {
    series.push({
      id: "price", name: "Price", type: "candlestick", data: cfg.ohlc,
      itemStyle: { color: col.up, color0: col.down, borderColor: col.up, borderColor0: col.down },
    });
  } else {
    series.push({
      id: "price", name: "Price", type: "line", data: cfg.close, showSymbol: false,
      lineStyle: { color: col.mauve, width: 2 },
      areaStyle: { color: new echarts.graphic.LinearGradient(0, 0, 0, 1, [
        { offset: 0, color: col.mauve + "55" }, { offset: 1, color: col.mauve + "00" },
      ]) },
    });
  }
  (cfg.overlays || []).forEach(function (o) {
    series.push({
      name: o.name, type: "line", data: o.values, showSymbol: false, connectNulls: false,
      lineStyle: { color: c(o.color), width: o.dashed ? 1 : 1.5, type: o.dashed ? "dashed" : "solid" },
      emphasis: { disabled: true },
    });
  });
  // What StockChart.tick moves: copies, so the next redraw starts clean.
  var data = {
    times: times.slice(), dates: dates.slice(), ohlc: cfg.ohlc.map(function (o) { return o.slice(); }),
    close: cfg.close.slice(), volume: cfg.volume.slice(), up: cfg.up.slice(),
    step: cfg.step || 86400, intraday: !!cfg.intraday, style: cfg.style, axes: grids.length, col: col,
    measureUnit: cfg.measureUnit || "",
  };
  series.push({
    id: "volume", name: "Volume", type: "bar", xAxisIndex: 1, yAxisIndex: 1, data: volumeData(data, col),
  });
  if (hasLower) {
    (cfg.lower.bars || []).forEach(function (b) {
      series.push({
        name: b.name, type: "bar", xAxisIndex: 2, yAxisIndex: 2, data: b.values.map(function (v) {
          return { value: v, itemStyle: { color: (v >= 0 ? col.up : col.down) + "aa" } };
        }),
      });
    });
    (cfg.lower.lines || []).forEach(function (l) {
      series.push({
        name: l.name, type: "line", xAxisIndex: 2, yAxisIndex: 2, data: l.values, showSymbol: false,
        lineStyle: { color: c(l.color), width: 1.5 },
        markLine: l.levels ? {
          silent: true, symbol: "none", label: { show: false },
          lineStyle: { color: col.faint, type: "dashed" },
          data: l.levels.map(function (y) { return { yAxis: y }; }),
        } : undefined,
      });
    });
  }

  // Drawings live on an empty series so they move with zoom and pan.
  series.push({
    id: "drawings", name: "Drawings", type: "line", data: [], silent: false,
    markLine: { symbol: ["none", "none"], animation: false, silent: false, data: drawingLines(id, data, col),
      emphasis: { lineStyle: { width: 3 } } },
  });

  var allAxes = grids.map(function (_, i) { return i; });
  chart.setOption({
    animation: false,
    backgroundColor: "transparent",
    textStyle: { color: col.text },
    grid: grids,
    xAxis: xAxes,
    yAxis: yAxes,
    axisPointer: { link: [{ xAxisIndex: "all" }], label: { backgroundColor: col.grid, color: col.text } },
    tooltip: {
      // Kept inside the chart: on a phone it ran off the screen's edge.
      trigger: "axis", axisPointer: { type: "cross" }, confine: true,
      backgroundColor: col.base, borderColor: col.grid, textStyle: { color: col.text, fontSize: 12 },
      formatter: function (params) {
        var lines = ["<b>" + esc(params[0].axisValue) + "</b>"];
        params.forEach(function (p) {
          var v = p.value;
          if (v === null || v === undefined || v === "-") return;
          if (p.seriesType === "candlestick") {
            var d = p.data;
            lines.push("O " + fmt(d[1]) + " · H " + fmt(d[4]) + " · L " + fmt(d[3]) + " · C " + fmt(d[2]));
          } else if (p.seriesName === "Volume") {
            lines.push("Volume " + compact(typeof v === "object" ? v.value : v));
          } else {
            lines.push(p.marker + esc(p.seriesName) + " " + fmt(typeof v === "object" ? v.value : v));
          }
        });
        return lines.join("<br/>");
      },
    },
    dataZoom: [
      // The wheel scrolls the page; Ctrl+wheel (or pinch) zooms, drag pans.
      { type: "inside", xAxisIndex: allAxes, start: zoom.start, end: zoom.end,
        zoomOnMouseWheel: "ctrl", moveOnMouseWheel: false, moveOnMouseMove: true },
      { type: "slider", xAxisIndex: allAxes, start: zoom.start, end: zoom.end, bottom: 4, height: 18,
        borderColor: col.grid, fillerColor: col.mauve + "22", handleStyle: { color: col.mauve },
        textStyle: { color: col.faint }, dataBackground: { lineStyle: { color: col.faint }, areaStyle: { color: col.grid } } },
    ],
    series: series,
  }, true);
  chart.__data = data;
  attachTools(chart, id);

  function fmt(v) { return Number(v).toLocaleString(undefined, { maximumFractionDigits: 2 }); }
  function esc(t) {
    return String(t).replace(/[&<>"']/g, function (ch) {
      return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[ch];
    });
  }
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

function compact(v) {
  var a = Math.abs(v);
  if (a >= 1e9) return (v / 1e9).toFixed(2) + "B";
  if (a >= 1e6) return (v / 1e6).toFixed(2) + "M";
  if (a >= 1e3) return (v / 1e3).toFixed(1) + "K";
  return String(Math.round(v * 100) / 100);
}
