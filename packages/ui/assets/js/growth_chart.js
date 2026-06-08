window.GrowthChart = window.GrowthChart || {};
window.GrowthChart.init = function (id, cfg) {
  var style = getComputedStyle(document.documentElement);
  function v(name) {
    return style.getPropertyValue(name).trim();
  }
  var colors = {
    text: v("--color-ctp-text"),
    subtext0: v("--color-ctp-subtext0"),
    overlay0: v("--color-ctp-overlay0"),
    surface0: v("--color-ctp-surface0"),
    surface1: v("--color-ctp-surface1"),
    surface2: v("--color-ctp-surface2"),
    base: v("--color-ctp-base"),
    mantle: v("--color-ctp-mantle"),
    green: v("--color-ctp-green"),
    red: v("--color-ctp-red"),
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
      '<span style="font-size:11px;color:' +
      colors.subtext0 +
      '">' +
      params[0].name +
      "</span>";
    var rows = params.map(function (p) {
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
        ';font-size:11px">' +
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

  function initChart() {
    var el = document.getElementById(id);
    if (!el) {
      console.error("[GrowthChart] #" + id + " not found");
      requestAnimationFrame(initChart);
      return;
    }

    if (el.__chart) {
      el.__chart.dispose();
      el.__chart = null;
    }
    var chart = echarts.init(el, null, { renderer: "canvas" });
    el.__chart = chart;

    var series = cfg.series.map(function (s) {
      return {
        name: s.name,
        type: "line",
        data: s.values,
        smooth: 0.3,
        symbol: "none",
        lineStyle: { color: s.color, width: 2 },
        areaStyle: {
          color: {
            type: "linear",
            x: 0,
            y: 0,
            x2: 0,
            y2: 1,
            colorStops: [
              { offset: 0, color: hexToRgba(s.color, 0.22) },
              { offset: 1, color: "rgba(0,0,0,0)" },
            ],
          },
        },
        markLine: {
          silent: true,
          symbol: ["none", "none"],
          lineStyle: { color: colors.surface1, type: "dashed", width: 1 },
          label: { show: false },
          data: [{ yAxis: 0 }],
        },
      };
    });

    var option = {
      backgroundColor: "transparent",
      legend: {
        show: cfg.showLegend,
        data: cfg.series.map(function (s) {
          return s.name;
        }),
        bottom: 0,
        textStyle: { color: colors.subtext0, fontSize: 11 },
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
        right: "60px",
        top: cfg.gridTop,
        bottom: cfg.gridBottom,
        containLabel: true,
      },
      xAxis: {
        type: "category",
        boundaryGap: false,
        data: cfg.labels,
        axisLine: { lineStyle: { color: colors.surface0 } },
        axisTick: { show: false },
        axisLabel: { color: colors.overlay0, fontSize: 11, interval: "auto" },
      },
      yAxis: {
        type: "value",
        position: "right",
        axisLabel: {
          color: colors.overlay0,
          fontSize: 11,
          formatter: function (val) {
            return val.toFixed(1) + "%";
          },
        },
        splitLine: { lineStyle: { color: colors.mantle, type: "dashed" } },
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

    chart.setOption(option);

    if (!el.__onresize) {
      el.__onresize = function () {
        if (el.__chart) el.__chart.resize();
      };
      window.addEventListener("resize", el.__onresize);
    }
  }

  function bootstrap() {
    requestAnimationFrame(initChart);
  }

  if (typeof echarts !== "undefined") {
    bootstrap();
  } else {
    var s = document.createElement("script");
    s.src = "https://cdn.jsdelivr.net/npm/echarts@5/dist/echarts.min.js";
    s.onload = bootstrap;
    s.onerror = function () {
      console.error("[GrowthChart] failed to load ECharts CDN");
    };
    document.head.appendChild(s);
  }
};
