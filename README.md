# Akhsakov Finance

แอปติดตามพอร์ตการลงทุนส่วนตัว เขียนด้วย Rust + [Dioxus 0.7](https://dioxuslabs.com/learn/0.7) ใช้ได้ทั้งเว็บ เดสก์ท็อป และมือถือ ข้อมูลเก็บใน SQLite บนเครื่องที่รันเซิร์ฟเวอร์ ราคาดึงจาก Yahoo Finance หน้าจอมีภาษาไทยและอังกฤษ

รายการฟีเจอร์ทั้งหมดอยู่ใน [docs/FEATURES.md](docs/FEATURES.md) วิธีเชื่อมต่อผู้ช่วย AI อยู่ใน [docs/AI_CONNECTOR.md](docs/AI_CONNECTOR.md)

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
- **AI connector**: MCP แบบ provider-neutral พร้อมตัวอย่างสำหรับหลายไคลเอนต์ และรองรับโมเดลผ่าน API ที่เข้ากันได้กับ OpenAI เพื่อบริหารพอร์ตเงินสมมติ
- ล็อกอินหลายผู้ใช้, 13 สกุลเงินแสดงผล, ธีม Catppuccin, โหมด Lite, คีย์ลัดแบบ Vim

## ติดตั้ง

ติดตั้งให้ผู้ใช้คนปัจจุบันจาก GitHub release ล่าสุด ไม่ต้องใช้ sudo หรือสิทธิ์ admin

**Linux (x86_64) และ macOS**

```sh
curl -fsSL https://raw.githubusercontent.com/Ax-47/akhsakov_finance/main/installer.sh | bash
```

**Windows** (PowerShell)

```powershell
irm https://raw.githubusercontent.com/Ax-47/akhsakov_finance/main/installer.ps1 | iex
```

**Android**: ดาวน์โหลด `akhsakov-finance-android-arm64.apk` จากหน้า [Releases](https://github.com/Ax-47/akhsakov_finance/releases) มาติดตั้งบนมือถือ เปิดแอปครั้งแรกจะถามที่อยู่เซิร์ฟเวอร์ ให้รัน `akhsakov-finance run server --lan` บนคอมพิวเตอร์ (มือถือกับคอมต้องอยู่ Wi-Fi เดียวกัน) แล้วกรอกที่อยู่ที่คำสั่งแสดง เช่น `http://192.168.1.20:8080` เปลี่ยนทีหลังได้ใน Settings → Server

ตัวแอปเดสก์ท็อปเปิดเซิร์ฟเวอร์ของตัวเองที่ `127.0.0.1:8080` แล้วปิดตามเมื่อปิดหน้าต่าง ข้อมูลอยู่ที่

| ระบบ | ข้อมูล (`akhsakov_finance.db`, `server.log`) | ตัวแอป |
|---|---|---|
| Linux | `~/.local/share/akhsakov-finance/` | อยู่ในโฟลเดอร์เดียวกัน (`app/`) และมีไอคอนในเมนูแอป |
| macOS | `~/Library/Application Support/akhsakov-finance/` | `~/Applications/AkhsakovFinance.app` |
| Windows | `%LOCALAPPDATA%\akhsakov-finance\` | `%LOCALAPPDATA%\Programs\AkhsakovFinance\` และใน Start menu |

### คำสั่ง `akhsakov-finance`

```sh
akhsakov-finance                        # เปิดแอป
akhsakov-finance run server             # รันแค่เซิร์ฟเวอร์ (ให้เครื่องนี้เท่านั้นเข้าได้)
akhsakov-finance run server --lan       # ให้มือถือและเครื่องอื่นใน Wi-Fi เดียวกันเข้าได้ (--port เปลี่ยนพอร์ต)
akhsakov-finance update                 # ติดตั้ง release ล่าสุด (--version <tag> เลือกเวอร์ชัน)
akhsakov-finance uninstall              # ถอนการติดตั้ง เก็บข้อมูลไว้ (--purge ลบข้อมูลด้วย)
akhsakov-finance version
```

- ถ้าเปิด `run server` ไว้ก่อน แอปเดสก์ท็อปจะใช้เซิร์ฟเวอร์ตัวนั้นแทนการเปิดของตัวเอง ใช้ข้อมูลชุดเดียวกับมือถือได้
- ก่อนใช้ `--lan` ให้สร้างบัญชีใน Settings → Security ก่อน ไม่งั้นใครใน Wi-Fi เดียวกันก็เปิดข้อมูลได้ ครั้งแรกระบบอาจถามเรื่อง firewall ให้กดอนุญาต
- ติดตั้งจากไฟล์ที่ build เอง: `./installer.sh --from <ไฟล์ .AppImage หรือ .zip>` หรือ `./installer.ps1 -From <ไฟล์ -setup.exe>`
- ย้ายข้อมูลจากตอนรัน `dx serve`: copy `akhsakov_finance.db` ไปไว้ในโฟลเดอร์ข้อมูลตามตารางด้านบนตอนแอปปิดอยู่
- Linux ต้องมี WebKitGTK 4.1 (Arch: `sudo pacman -S webkit2gtk-4.1`) ถ้าขาดอะไร installer จะบอก
- macOS: ไฟล์ `.zip` ที่ดาวน์โหลดผ่านเบราว์เซอร์จะติด quarantine เพราะแอปไม่ได้ sign ถ้าเปิดไม่ขึ้นให้ใช้ installer หรือรัน `xattr -dr com.apple.quarantine AkhsakovFinance.app`

### ออก release

push tag ที่ขึ้นต้นด้วย `v` (เช่น `git tag v0.2.0 && git push origin v0.2.0`) แล้ว workflow [Release](.github/workflows/release.yml) จะ build ทุกแพลตฟอร์ม ใส่เซิร์ฟเวอร์ไว้ในแต่ละ bundle และสร้าง GitHub release พร้อม `SHA256SUMS` ให้ installer ใช้

ถ้าอยากให้อัปเดตแอป Android ทับของเดิมได้ ต้อง sign ด้วย key เดิมทุกครั้ง: ตั้ง secret `ANDROID_KEYSTORE` (ไฟล์ .jks แบบ base64), `ANDROID_KEYSTORE_PASSWORD` และ `ANDROID_KEY_ALIAS` ถ้าไม่ตั้ง APK จะ sign ด้วย debug key ที่เปลี่ยนทุก build ต้องลบแอปเก่าก่อนติดตั้งเวอร์ชันใหม่

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

### Docker

Docker image รวมเว็บและเซิร์ฟเวอร์แบบ release ไว้ด้วยกัน ข้อมูล SQLite จะอยู่ในโฟลเดอร์ `data/` บนเครื่อง และบังคับให้ล็อกอินตั้งแต่แรก

```sh
mkdir -p data
docker compose up -d
```

เปิด `http://127.0.0.1:8080` แล้วสร้างบัญชีแรก ดูสถานะและ log หรือหยุดเซิร์ฟเวอร์ได้ด้วย

```sh
docker compose ps
docker compose logs -f app
docker compose down
```

อัปเดตเป็น release ล่าสุดแล้วเปิด container ใหม่ด้วย

```sh
docker compose pull
docker compose up -d
```

ค่าเริ่มต้นใช้ `ghcr.io/ax-47/akhsakov_finance:latest` ซึ่งมาจาก release tag ล่าสุด เลือกเวอร์ชันตายตัวหรือ build ล่าสุดจาก branch `main` ได้ด้วย `AKHSAKOV_IMAGE`

```sh
AKHSAKOV_IMAGE=ghcr.io/ax-47/akhsakov_finance:0.2.0 docker compose up -d
AKHSAKOV_IMAGE=ghcr.io/ax-47/akhsakov_finance:edge docker compose up -d
```

สำหรับผู้ดูแลโปรเจกต์: หลัง workflow publish package ครั้งแรก ให้เปิดหน้า Package settings ใน GitHub แล้วเปลี่ยน visibility เป็น Public หนึ่งครั้ง เพื่อให้ผู้ใช้ pull image ได้โดยไม่ต้องล็อกอิน

ถ้าต้องการ build source ใน checkout นี้เอง ให้ใช้ Compose override สำหรับ local build

```sh
docker compose -f compose.yaml -f compose.local.yaml up --build -d
```

คำสั่งนี้ build `akhsakov-finance:local` จาก `Dockerfile` แล้วใช้ค่า environment, port และ data volume ชุดเดียวกับ Compose ปกติ

ตั้งพอร์ตหรือที่เก็บข้อมูลเองได้โดยใส่ตัวแปรหน้าคำสั่ง เช่น

```sh
AKHSAKOV_PORT=8090 AKHSAKOV_DATA_DIR=/srv/akhsakov-finance docker compose up -d
```

ตั้ง `AKHSAKOV_REQUIRE_LOGIN=0` ได้ถ้าต้องการพฤติกรรมเหมือน `dx serve` แต่ไม่ควรทำเมื่อเครื่องอื่นเข้าถึงพอร์ตนี้ได้ ถ้าจะเปิดผ่านอินเทอร์เน็ต ให้วาง reverse proxy ที่มี HTTPS ไว้ข้างหน้า

สำรองข้อมูลผ่าน Settings → Backup ได้ขณะที่ระบบทำงานอยู่ ถ้าจะ copy โฟลเดอร์ข้อมูลโดยตรง ให้รัน `docker compose down` ก่อนเพื่อให้ไฟล์ SQLite และ WAL อยู่ในสถานะที่สอดคล้องกัน การลบ container ไม่ลบข้อมูลในโฟลเดอร์บนเครื่อง

### เดสก์ท็อป

```sh
cd packages/desktop
dx serve --platform desktop
```

บน Linux ต้องมี WebKitGTK (`libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev`)

### มือถือ

แอปมือถือต่อกับเซิร์ฟเวอร์ที่รันอยู่บนเครื่องอื่น ถ้าไม่ได้ระบุตอน build แอปจะถามที่อยู่ตอนเปิดครั้งแรก (มือถือมองไม่เห็น `127.0.0.1` ของคอมพิวเตอร์ ส่วน Android emulator ใช้ `http://10.0.2.2:8080`) ระบุไว้ตอน build ได้แบบนี้

```sh
cd packages/mobile
AKHSAKOV_SERVER_URL=http://192.168.1.20:8080 dx serve --platform android
```

### ตัวแปรสภาพแวดล้อม

| ตัวแปร | ความหมาย |
|---|---|
| `AKHSAKOV_DB` | ไฟล์ฐานข้อมูล (ค่าเริ่มต้น `akhsakov_finance.db` ในโฟลเดอร์ที่รัน ส่วนแอปที่ติดตั้งใช้โฟลเดอร์ข้อมูลตามตารางในหัวข้อติดตั้ง) |
| `AKHSAKOV_REQUIRE_LOGIN` | `1` = บังคับล็อกอินตั้งแต่แรก แม้ยังไม่มีบัญชี |
| `AKHSAKOV_SERVER_URL` | ที่อยู่เซิร์ฟเวอร์ของแอปมือถือ (ใช้ตอน build ถ้าไม่ตั้ง แอปจะถามตอนเปิดครั้งแรก) |
| `AKHSAKOV_FAST_RENDERING` | `1` = ให้เดสก์ท็อปบน Wayland ใช้ GPU เต็มที่ (เร็วกว่า แต่บางเครื่องจอกระพริบ) |

## โครงสร้างโปรเจกต์

```
packages/
├─ web/       จุดเริ่มของเว็บ (route และเมนู)
├─ desktop/   จุดเริ่มของเดสก์ท็อป (แอปที่ติดตั้งเปิดเซิร์ฟเวอร์ของตัวเอง: src/local_server.rs)
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
