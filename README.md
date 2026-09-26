# Akhsakov Finance

แอปติดตามพอร์ตการลงทุนส่วนตัว เขียนด้วย Rust + [Dioxus 0.7](https://dioxuslabs.com/learn/0.7) ใช้ได้ทั้งเว็บ เดสก์ท็อป และมือถือ ข้อมูลเก็บใน SQLite บนเครื่องที่รันเซิร์ฟเวอร์ ราคาดึงจาก Yahoo Finance หน้าจอมีภาษาไทยและอังกฤษ

รายการฟีเจอร์ทั้งหมดอยู่ใน [docs/FEATURES.md](docs/FEATURES.md) วิธีต่อกับ Claude อยู่ใน [docs/CLAUDE_CONNECTOR.md](docs/CLAUDE_CONNECTOR.md)

## ฟีเจอร์หลัก

- **พอร์ตโฟลิโอ**: หลายพอร์ต ธุรกรรมซื้อ/ขาย/ปันผล/แตกหุ้น/ฝาก/ถอน พร้อมค่าธรรมเนียมและสกุลเงิน แท็บ Overview, Thesis, Income, Risk, Plan และ Activity
- **สินทรัพย์ประเภทอื่น**: กองทุนรวม ทองคำ เงินฝาก พันธบัตร คริปโต กรอกราคาเองได้สำหรับของที่ไม่มีราคาตลาด และระบุกองทุนเป็น SSF / RMF / Thai ESG
- **ภาษีไทย**: สรุปรายปีเป็นเงินบาท ได้แก่ ปันผลและภาษีหัก ณ ที่จ่าย, เครดิตภาษีเงินปันผล, เงินได้จากต่างประเทศ และวงเงินลดหย่อน SSF / RMF / Thai ESG ที่ยังเหลือ
- **แผนลงทุนรายเดือน (DCA)**: เตือนเมื่อถึงวัน และบันทึกการซื้อได้ในคลิกเดียว
- **ความเสี่ยงและการวางแผน**: VaR / expected shortfall, stress test, correlation, CAPM, rebalance, FIFO tax lots, เป้าหมายการเงิน
- **หุ้นรายตัว ตลาด Screener ปฏิทิน เศรษฐกิจ Backtest** และบทเรียนใน **Learn**
- **Watchlist และแจ้งเตือน**: alert ราคาและพอร์ต, alert ทางเทคนิคัล (RSI, ราคาตัดเส้น SMA, 52-week high/low, วันประกาศงบ, วัน XD) เซิร์ฟเวอร์เช็กทุกนาทีและส่งเข้า ntfy / Telegram / webhook
- **รายงานประจำเดือน**: ดูย้อนหลังได้ 12 เดือน ดาวน์โหลดหรือพิมพ์เป็น PDF ได้ และตั้งให้ส่งเองทุกวันที่ 1
- **ข้อมูลเข้า-ออก**: Import CSV พร้อม preset สำหรับ Streaming, Dime!, Webull, Interactive Brokers, Export CSV, Backup / Restore
- **โหมดออฟไลน์**: เก็บพอร์ต ราคา และข้อมูลหน้าไว้ในเครื่อง ถ้าต่อเซิร์ฟเวอร์ไม่ได้ก็ยังเปิดดูได้
- **มือถือ**: บนจอเล็กมีแถบเมนูด้านล่างแบบแอป และแสดงการแจ้งเตือนผ่านระบบของเครื่องได้
- **เชื่อมต่อ Claude (MCP)**: ให้ Claude อ่านพอร์ตและช่วยจด thesis ได้ และให้เงินสมมติกับ Claude ไปบริหารพอร์ตของตัวเองได้
- ล็อกอินหลายผู้ใช้, 13 สกุลเงินแสดงผล, ธีม Catppuccin, โหมด Lite, คีย์ลัดแบบ Vim

## เริ่มใช้งาน

ต้องมี Rust (stable) และ [Dioxus CLI](https://dioxuslabs.com/learn/0.7/getting_started/)

```sh
curl -sSL http://dioxus.dev/install.sh | sh
```

### เว็บ

```sh
cd packages/web
dx serve
```

เปิดที่อยู่ที่ `dx` แสดง (ปกติคือ `http://127.0.0.1:8080`) ครั้งแรกแอปจะสร้างพอร์ตตัวอย่างให้ ถ้าจะเปิดให้เครื่องอื่นเข้าได้ ให้สร้างบัญชีใน Settings ก่อน

### เดสก์ท็อป

```sh
cd packages/desktop
dx serve --platform desktop
```

บน Linux ต้องมี WebKitGTK (`libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev`)

### มือถือ

แอปมือถือต่อกับเซิร์ฟเวอร์ที่รันอยู่บนเครื่องอื่น ต้องบอกที่อยู่เซิร์ฟเวอร์ตอน build (มือถือมองไม่เห็น `127.0.0.1` ของคอมพิวเตอร์ ส่วน Android emulator ใช้ `http://10.0.2.2:8080`)

```sh
cd packages/mobile
AKHSAKOV_SERVER_URL=http://192.168.1.20:8080 dx serve --platform android
```

### ตัวแปรสภาพแวดล้อม

| ตัวแปร | ความหมาย |
|---|---|
| `AKHSAKOV_DB` | ไฟล์ฐานข้อมูล (ค่าเริ่มต้น `akhsakov_finance.db` ในโฟลเดอร์ที่รัน) |
| `AKHSAKOV_REQUIRE_LOGIN` | `1` = บังคับล็อกอินตั้งแต่แรก แม้ยังไม่มีบัญชี |
| `AKHSAKOV_SERVER_URL` | ที่อยู่เซิร์ฟเวอร์ของแอปมือถือ (ใช้ตอน build) |
| `AKHSAKOV_FAST_RENDERING` | `1` = ให้เดสก์ท็อปบน Wayland ใช้ GPU เต็มที่ (เร็วกว่า แต่บางเครื่องจอกระพริบ) |

## โครงสร้างโปรเจกต์

```
packages/
├─ web/       จุดเริ่มของเว็บ (route และเมนู)
├─ desktop/   จุดเริ่มของเดสก์ท็อป (เซิร์ฟเวอร์รันในแอปเดียวกัน)
├─ mobile/    จุดเริ่มของมือถือ
├─ ui/        หน้าจอและคอมโพเนนต์ที่ทุกแพลตฟอร์มใช้ร่วมกัน, คำแปลภาษาไทย (src/i18n/thai.rs)
├─ api/       server functions แยกเป็น bounded context (portfolio, quote, assets,
│             planning, reports, notifications, thesis, mcp …) แต่ละ context มี
│             repositories / infrastructures / services / controller
├─ dtos/      ข้อมูลที่ส่งระหว่าง client กับ server และการคำนวณที่ไม่พึ่งเครือข่าย
│             (ตำแหน่งถือครอง, ภาษีไทย, DCA, รายงานประจำเดือน, นำเข้า CSV …)
└─ types/     type พื้นฐาน เช่น TickerSymbol, AssetClass, Candle
```

## ตรวจสอบก่อนส่งโค้ด

ชุดเดียวกับที่ CI รัน:

```sh
cargo clippy --workspace --all-targets --features api/server,ui/server
cargo test --workspace --features api/server,ui/server
cargo check -p web --features web --target wasm32-unknown-unknown

# ถ้าแก้ class ของ Tailwind ต้อง build CSS ใหม่แล้ว commit ไฟล์ที่ได้
cd packages/ui && npm ci && npm run build:css
```
