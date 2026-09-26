# UI

This crate contains all shared components for the workspace. This is a great place to place any UI you would like to use in multiple platforms like a common `Button` or `Navbar` component.

```
ui/
├─ src/
│  ├─ lib.rs # The entrypoint for the ui crate
│  ├─ hero.rs # The Hero component that will be used in every platform
│  ├─ echo.rs # The shared echo component that communicates with the server
│  ├─ navbar.rs # The Navbar component that will be used in the layout of every platform's router
```

## Dependencies

Since this crate is shared between multiple platforms, it should not pull in any platform specific dependencies. For example, if you want to use the `web_sys` crate in the web build of your app, you should not add it to this crate. Instead, you should add platform specific dependencies to the [web](../web/Cargo.toml), [desktop](../desktop/Cargo.toml), or [mobile](../mobile/Cargo.toml) crates.

## Generated assets

Two files in `assets/` are built from sources in this crate and committed, so
`dx serve` works without Node. Rebuild them after changing their inputs
(`npm install` first):

| File | Built from | Command |
| --- | --- | --- |
| `assets/tailwind.css` | `input.css` and the classes used in `src/` | `npm run build:css` |
| `assets/js/echarts.min.js` | `js/echarts.entry.js` | `npm run build:echarts` |

`build:css` runs with `--optimize`, which flattens nested CSS rules and adds
vendor prefixes. Without it, WebViews older than Chrome 112 / Safari 16.5 /
WebKitGTK 2.42 drop every hover style and fall back to solid colours for
translucent ones.

The ECharts bundle is a slim build with only the charts and components the
chart scripts (`assets/js/*_chart.js`) use, about half the size of the full
library. If a chart starts using another ECharts chart type or component, add
it to `js/echarts.entry.js` and rebuild.

## Effects (Lite mode)

Settings → Appearance → Effects switches between full effects and **Lite**
(`src/perf.rs`): no animations, transitions or rolling numbers, and live
prices refresh every 3 seconds instead of every second. **Auto** (the default)
uses Lite when the device asks for reduced motion or looks low-end (2 or fewer
CPU threads, 2 GB or less memory, or data saver on). Lite tags `<html>` with
`lite`; the CSS rules for it are in `input.css`.
