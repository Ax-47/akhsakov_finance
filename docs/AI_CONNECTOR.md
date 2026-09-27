# AI connector และพอร์ตเงินสมมติ

Akhsakov Finance เปิด MCP server แบบ provider-neutral ที่ `/mcp` ผู้ช่วย AI ตัวใดที่รองรับ Streamable HTTP MCP สามารถอ่านพอร์ตและ thesis, เพิ่มบันทึก และจัดการพอร์ตเงินสมมติได้

## เปิดใช้ MCP

1. เปิด **Settings → Connect an AI assistant through MCP → Turn on** เพื่อสร้างคีย์ส่วนตัว
2. ใส่ที่อยู่ที่ไคลเอนต์เข้าถึงได้ ไคลเอนต์บนคลาวด์ต้องใช้ HTTPS ส่วนไคลเอนต์บนเครื่องเดียวกันใช้ `http://127.0.0.1:8080` ได้
3. เลือกตัวอย่าง Generic, Claude, Codex, Antigravity / `agy`, Cursor / VS Code หรือ Other แล้วคัดลอกค่าที่แสดง

ตัวเลือกเหล่านี้เป็นเพียงตัวอย่างการตั้งค่า ทุกตัวเชื่อมต่อ endpoint เดียวและเห็นเครื่องมือชุดเดียวกัน เซิร์ฟเวอร์ไม่เปลี่ยนพฤติกรรมตามชื่อไคลเอนต์หรือผู้ให้บริการโมเดล

### รูปแบบทั่วไป

- URL: `https://<server>/mcp`
- Header: `Authorization: Bearer <key>`
- สำหรับไคลเอนต์ที่ตั้ง header ไม่ได้: `https://<server>/mcp/<key>`

### ตัวอย่างไคลเอนต์

- Claude Code: ใช้คำสั่ง `claude mcp add --transport http ... --header "Authorization: Bearer ..."` ที่หน้า Settings สร้างให้
- Codex: เพิ่ม `[mcp_servers.akhsakov]` พร้อม `url` และ `http_headers` ลง `~/.codex/config.toml`
- Antigravity / `agy`: เพิ่ม `serverUrl` และ `Authorization` ใต้ `mcpServers` ใน `.agents/mcp_config.json` หรือไฟล์ global
- Cursor / VS Code: เพิ่ม HTTP server ในไฟล์ MCP JSON ของไคลเอนต์

## เครื่องมือ MCP

| เครื่องมือ | การทำงาน |
|---|---|
| `list_portfolios` | อ่านรายชื่อพอร์ตและหุ้นที่ถือ |
| `list_theses` | อ่าน thesis และรายการที่ยังไม่มี thesis |
| `get_thesis` | อ่าน thesis, บันทึกติดตาม และสถานะของหุ้นหนึ่งตัว |
| `save_thesis` | สร้างหรือแก้ thesis เมื่อผู้ใช้ขอหรือยืนยัน |
| `add_thesis_note` | เพิ่มบันทึกติดตามโดยระบุผู้เขียนเป็น AI |
| `get_quote` | อ่านราคาล่าสุด |
| `get_my_portfolio` | อ่านพอร์ตเงินสมมติที่ AI จัดการ |
| `place_order` | ซื้อขายเฉพาะในพอร์ตเงินสมมติ พร้อมบันทึกเหตุผล |

## Model connections

Settings รองรับ profile ของ API ที่เข้ากันได้กับ OpenAI Chat Completions โดยกรอกชื่อ, API base URL, model identifier และ API key เอง ไม่มี preset หรือโค้ดเฉพาะผู้ให้บริการ

เลือก profile และกำหนดกลยุทธ์ให้พอร์ตเงินสมมติ แล้วกด **Run AI trader** แอปจะส่งเฉพาะข้อมูลพอร์ตนั้นให้โมเดลและอนุญาตให้ซื้อขายเฉพาะพอร์ตนั้น รายการซื้อขายเกิดขึ้นทันทีด้วยเงินสมมติและเก็บ audit log ไว้

## ความปลอดภัย

- Connector ปิดอยู่จนกดเปิดใช้ และทุกคำขอต้องมีคีย์
- ใครมี MCP key สามารถใช้เครื่องมือเขียนได้ ถ้าคีย์หลุดให้สร้างใหม่ทันที
- API key ถูกเก็บแยก ไม่แสดงกลับใน UI และไม่รวมในไฟล์ backup
- พอร์ตเงินสมมติไม่นับรวมยอด, รายงาน, ภาษี หรือการแจ้งเตือนของพอร์ตจริง
- การหยุด AI trading ไม่ลบพอร์ตหรือประวัติ
