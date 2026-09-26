// Technical stock chart: candles or a line with overlays (moving averages,
// Bollinger bands), volume, and an RSI or MACD pane. Rust computes every
// series; this file only draws. Loads ECharts through GrowthChart.ready.
window.StockChart = window.StockChart || {};
window.StockChart.latest = window.StockChart.latest || {};
window.StockChart.charts = window.StockChart.charts || {};

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

function draw(el, id, cfg) {
  var css = getComputedStyle(el);
  function c(name) { return css.getPropertyValue("--catppuccin-color-" + name).trim(); }
  var col = {
    text: c("text"), sub: c("subtext0"), faint: c("overlay0"), grid: c("surface0"),
    up: c("green"), down: c("red"), mauve: c("mauve"), base: c("mantle"),
  };
  var chart = window.StockChart.charts[id];
  if (!chart || chart.isDisposed()) {
    chart = echarts.init(el, null, { renderer: "canvas" });
    window.StockChart.charts[id] = chart;
    new ResizeObserver(function () { chart.resize(); }).observe(el);
  }

  var hasLower = cfg.lower && cfg.lower.kind !== "none";
  var grids = [
    { left: 56, right: 16, top: 12, height: hasLower ? "52%" : "66%" },
    { left: 56, right: 16, top: hasLower ? "60%" : "72%", height: "10%" },
  ];
  if (hasLower) grids.push({ left: 56, right: 16, top: "74%", height: "14%" });

  function xAxis(i) {
    return {
      type: "category", gridIndex: i, data: cfg.dates, boundaryGap: true,
      axisLine: { lineStyle: { color: col.grid } }, axisTick: { show: false },
      axisLabel: { show: i === grids.length - 1, color: col.faint, fontSize: 12 },
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
    yAxes.push(cfg.lower.kind === "rsi"
      ? yAxis(2, { min: 0, max: 100, interval: 50, scale: false })
      : yAxis(2, { splitNumber: 2 }));
  }

  var series = [];
  if (cfg.style === "candle") {
    series.push({
      name: "Price", type: "candlestick", data: cfg.ohlc,
      itemStyle: { color: col.up, color0: col.down, borderColor: col.up, borderColor0: col.down },
    });
  } else {
    series.push({
      name: "Price", type: "line", data: cfg.close, showSymbol: false,
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
  series.push({
    name: "Volume", type: "bar", xAxisIndex: 1, yAxisIndex: 1, data: cfg.volume.map(function (v, i) {
      return { value: v, itemStyle: { color: (cfg.up[i] ? col.up : col.down) + "88" } };
    }),
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
      trigger: "axis", axisPointer: { type: "cross" },
      backgroundColor: col.base, borderColor: col.grid, textStyle: { color: col.text, fontSize: 12 },
      formatter: function (params) {
        var lines = ["<b>" + params[0].axisValue + "</b>"];
        params.forEach(function (p) {
          var v = p.value;
          if (v === null || v === undefined || v === "-") return;
          if (p.seriesType === "candlestick") {
            var d = p.data;
            lines.push("O " + fmt(d[1]) + " · H " + fmt(d[4]) + " · L " + fmt(d[3]) + " · C " + fmt(d[2]));
          } else if (p.seriesName === "Volume") {
            lines.push("Volume " + compact(typeof v === "object" ? v.value : v));
          } else {
            lines.push(p.marker + p.seriesName + " " + fmt(typeof v === "object" ? v.value : v));
          }
        });
        return lines.join("<br/>");
      },
    },
    dataZoom: [
      // The wheel scrolls the page; Ctrl+wheel (or pinch) zooms, drag pans.
      { type: "inside", xAxisIndex: allAxes, start: cfg.zoomStart, end: 100,
        zoomOnMouseWheel: "ctrl", moveOnMouseWheel: false, moveOnMouseMove: true },
      { type: "slider", xAxisIndex: allAxes, start: cfg.zoomStart, end: 100, bottom: 4, height: 18,
        borderColor: col.grid, fillerColor: col.mauve + "22", handleStyle: { color: col.mauve },
        textStyle: { color: col.faint }, dataBackground: { lineStyle: { color: col.faint }, areaStyle: { color: col.grid } } },
    ],
    series: series,
  }, true);

  function fmt(v) { return Number(v).toLocaleString(undefined, { maximumFractionDigits: 2 }); }
  function compact(v) {
    var a = Math.abs(v);
    if (a >= 1e9) return (v / 1e9).toFixed(2) + "B";
    if (a >= 1e6) return (v / 1e6).toFixed(2) + "M";
    if (a >= 1e3) return (v / 1e3).toFixed(1) + "K";
    return String(v);
  }
}
