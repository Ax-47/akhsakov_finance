window.GrowthChart = window.GrowthChart || {};
// Newest config per chart id: an older draw that finishes late is skipped.
window.GrowthChart.latest = window.GrowthChart.latest || {};

// ECharts ships with the app (assets/js/echarts.min.js; the App root sets
// window.ECHARTS_URL). It's loaded the first time a chart draws, falling
// back to the CDN if the bundled copy can't be loaded.
window.GrowthChart.ready =
  window.GrowthChart.ready ||
  function () {
    if (typeof echarts !== "undefined") return Promise.resolve();
    if (!window.GrowthChart.loading) {
      var load = function (src) {
        return new Promise(function (resolve, reject) {
          var s = document.createElement("script");
          s.src = src;
          s.onload = resolve;
          s.onerror = reject;
          document.head.appendChild(s);
        });
      };
      window.GrowthChart.loading = new Promise(function (resolve) {
        // The URL is set as the app starts; give it a moment if needed.
        var waited = 0;
        (function wait() {
          if (window.ECHARTS_URL || (waited += 50) > 2000) return resolve();
          setTimeout(wait, 50);
        })();
      })
        .then(function () {
          return window.ECHARTS_URL ? load(window.ECHARTS_URL) : Promise.reject();
        })
        .catch(function () {
          return load("https://cdn.jsdelivr.net/npm/echarts@5/dist/echarts.min.js");
        })
        .catch(function (e) {
          window.GrowthChart.loading = null;
          throw new Error("[GrowthChart] ECharts unavailable");
        });
    }
    return window.GrowthChart.loading;
  };

window.GrowthChart.init = function (id, cfg) {
  window.GrowthChart.latest[id] = cfg;
  // Theme colours come from the chart's container (the themed page sets
  // the --catppuccin-color-* variables).
  var style = getComputedStyle(document.getElementById(id) || document.documentElement);
  function v(name) {
    return style.getPropertyValue("--catppuccin-color-" + name).trim();
  }
  // Series colours may be "var(--catppuccin-color-…)".
  function resolve(c) {
    var m = /^var\((--[\w-]+)\)$/.exec((c || "").trim());
    return m ? style.getPropertyValue(m[1]).trim() || c : c;
  }
  (cfg.series || []).forEach(function (s) { s.color = resolve(s.color); });
  var colors = {
    text: v("text"),
    subtext0: v("subtext0"),
    overlay0: v("overlay0"),
    surface0: v("surface0"),
    surface1: v("surface1"),
    surface2: v("surface2"),
    base: v("base"),
    mantle: v("mantle"),
    crust: v("crust"),
    green: v("green"),
    red: v("red"),
  };

  function hexToRgba(hex, alpha) {
    var c = hex.trim();
    if (!c.startsWith("#")) return c;
    if (c.length === 4) c = "#" + c[1] + c[1] + c[2] + c[2] + c[3] + c[3];
    var r = parseInt(c.slice(1, 3), 16);
    var g = parseInt(c.slice(3, 5), 16);
    var b = parseInt(c.slice(5, 7), 16);
    return "rgba(" + r + "," + g + "," + b + "," + alpha + ")";
  }

  function buildTooltip(params) {
    var header =
      '<span style="font-size:12px;color:' +
      colors.subtext0 +
      '">' +
      params[0].name +
      "</span>";
    var rows = params
      .filter(function (p) {
        return p.value != null;
      })
      .map(function (p) {
      var val = p.value;
      var sign = val >= 0 ? "+" : "";
      var col = val >= 0 ? colors.green : colors.red;
      return (
        "<br/>" +
        '<span style="display:inline-block;width:8px;height:8px;' +
        "border-radius:50%;background:" +
        p.color +
        ';margin-right:6px"></span>' +
        '<span style="color:' +
        colors.subtext0 +
        ';font-size:12px">' +
        p.seriesName +
        ": </span>" +
        '<span style="color:' +
        col +
        ';font-weight:700;font-size:13px">' +
        sign +
        val.toFixed(2) +
        "%</span>"
      );
    });
    return header + rows.join("");
  }

  var tries = 0;
  function initChart() {
    if (window.GrowthChart.latest[id] !== cfg) return; // superseded or removed
    var el = document.getElementById(id);
    if (!el) {
      // Not mounted yet; give up after ~1s instead of spinning every frame
      // forever when the chart was removed before it drew.
      if (++tries < 60) requestAnimationFrame(initChart);
      return;
    }

    // Reuse the chart: disposing and re-creating it on every redraw was slow.
    var chart = el.__chart && !el.__chart.isDisposed() ? el.__chart : echarts.init(el, null, { renderer: "canvas" });
    el.__chart = chart;

    // Broker-style lines: thin, no fill except a faint wash under the first
    // series, a dot and a colour-filled % pill at the end of each line.
    var last = cfg.labels.length - 1;
    var series = cfg.series.map(function (s, idx) {
      return {
        name: s.name,
        type: "line",
        data: s.values,
        smooth: 0.2,
        showSymbol: true,
        showAllSymbol: true,
        symbol: "circle",
        symbolSize: function (_value, p) {
          return p.dataIndex === last ? 9 : 0;
        },
        itemStyle: { color: s.color, borderColor: colors.base, borderWidth: 2 },
        lineStyle: { color: s.color, width: 2 },
        emphasis: { focus: "series" },
        endLabel: {
          show: true,
          distance: 10,
          formatter: function (p) {
            var v = Array.isArray(p.value) ? p.value[1] : p.value;
            return (v >= 0 ? "+" : "") + Number(v).toFixed(2) + "%";
          },
          color: colors.crust,
          backgroundColor: s.color,
          borderRadius: 999,
          padding: [4, 10],
          fontSize: 12,
          fontWeight: 600,
        },
        labelLayout: { moveOverlap: "shiftY" },
        areaStyle:
          idx === 0
            ? {
                color: {
                  type: "linear",
                  x: 0,
                  y: 0,
                  x2: 0,
                  y2: 1,
                  colorStops: [
                    { offset: 0, color: hexToRgba(s.color, 0.12) },
                    { offset: 1, color: "rgba(0,0,0,0)" },
                  ],
                },
              }
            : undefined,
        markLine:
          idx === 0
            ? {
                silent: true,
                symbol: ["none", "none"],
                lineStyle: { color: colors.surface2, type: "dashed", width: 1 },
                label: { show: false },
                data: [{ yAxis: 0 }],
              }
            : undefined,
      };
    });

    var option = {
      backgroundColor: "transparent",
      legend: {
        show: false,
        data: cfg.series.map(function (s) {
          return s.name;
        }),
        bottom: 0,
        textStyle: { color: colors.subtext0, fontSize: 12 },
        icon: "circle",
        itemWidth: 8,
        itemHeight: 8,
      },
      tooltip: {
        trigger: "axis",
        axisPointer: {
          type: "line",
          lineStyle: { color: colors.surface2, type: "dashed" },
        },
        formatter: buildTooltip,
        backgroundColor: colors.base,
        borderColor: colors.surface0,
        textStyle: { color: colors.text },
        extraCssText: "border-radius:8px;padding:8px 12px;",
      },
      grid: {
        left: "10px",
        right: "84px",
        top: cfg.gridTop,
        bottom: cfg.gridBottom,
        containLabel: true,
      },
      xAxis: {
        type: "category",
        boundaryGap: false,
        data: cfg.labels,
        axisLine: { show: false },
        axisTick: { show: false },
        axisLabel: {
          color: colors.overlay0,
          fontSize: 12,
          interval: "auto",
          hideOverlap: true,
        },
      },
      yAxis: {
        type: "value",
        position: "right",
        scale: true,
        axisLabel: { show: false },
        splitLine: { show: false },
        axisLine: { show: false },
        axisTick: { show: false },
      },
      series: series,
    };

    if (cfg.title) {
      option.title = {
        text: cfg.title,
        left: "center",
        textStyle: { color: colors.text, fontSize: 13, fontWeight: "normal" },
      };
    }

    chart.setOption(option, true);

    if (!el.__onresize) {
      el.__onresize = function () {
        if (el.__chart) el.__chart.resize();
      };
      window.addEventListener("resize", el.__onresize);
      // Also when the element itself changes size, e.g. an off-screen card
      // being laid out for the first time as it scrolls into view.
      if (window.ResizeObserver) {
        el.__observer = new ResizeObserver(el.__onresize);
        el.__observer.observe(el);
      }
    }
  }

  function bootstrap() {
    requestAnimationFrame(initChart);
  }

  window.GrowthChart.ready().then(bootstrap, function (e) {
    console.error(e.message);
  });
};

// Frees a chart whose component unmounted. Without this, every chart ever
// drawn stayed alive (window listener + ECharts instance), so the app got
// slower the longer it ran.
window.GrowthChart.dispose = function (id) {
  delete window.GrowthChart.latest[id];
  var el = document.getElementById(id);
  if (!el) return;
  if (el.__onresize) window.removeEventListener("resize", el.__onresize);
  if (el.__observer) el.__observer.disconnect();
  if (el.__chart && !el.__chart.isDisposed()) el.__chart.dispose();
  el.__chart = el.__onresize = el.__observer = null;
};
