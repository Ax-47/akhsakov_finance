// Slim ECharts build: only what growth_chart.js and stock_chart.js draw.
// The full bundle was ~1 MB of JavaScript to parse before the first chart;
// this is about a third of that. Rebuild with `npm run build:echarts` and
// add the chart or component here if a chart starts using a new one.
import { graphic, init, use } from "echarts/core";
import { BarChart, CandlestickChart, CustomChart, LineChart } from "echarts/charts";
import {
  AxisPointerComponent,
  DataZoomInsideComponent,
  DataZoomSliderComponent,
  GridComponent,
  MarkAreaComponent,
  MarkLineComponent,
  MarkPointComponent,
  TitleComponent,
  TooltipComponent,
} from "echarts/components";
import { LabelLayout } from "echarts/features";
import { CanvasRenderer } from "echarts/renderers";

use([
  LineChart,
  BarChart,
  CandlestickChart,
  // Open-high-low-close bars on the stock chart.
  CustomChart,
  GridComponent,
  TooltipComponent,
  AxisPointerComponent,
  DataZoomInsideComponent,
  DataZoomSliderComponent,
  MarkLineComponent,
  // The viewer's trades, and the ground under trading outside regular hours.
  MarkPointComponent,
  MarkAreaComponent,
  TitleComponent,
  LabelLayout,
  CanvasRenderer,
]);

// The chart scripts only call `echarts.init` and `echarts.graphic`.
window.echarts = { init, graphic };
